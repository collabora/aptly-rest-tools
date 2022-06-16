/// Scan directories for dsc or changes files
///
use futures::Stream;
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    Semaphore,
};

use crate::{
    changes::{Changes, ChangesError},
    dsc::{Dsc, DscError},
};
use std::{path::PathBuf, sync::Arc};

pub enum Found {
    Dsc(Dsc),
    Changes(Changes),
}

#[derive(thiserror::Error, Debug)]
pub enum ScannerError {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Parsing changes: {0}")]
    Changes(#[from] ChangesError),
    #[error("Parsing dsc: {0}")]
    Dsc(#[from] DscError),
}

pub struct Scanner {
    state: ScannerState,
}

impl Scanner {
    pub fn new(path: PathBuf) -> Self {
        let state = ScannerState::Init(path);
        Scanner { state }
    }
}

impl Stream for Scanner {
    type Item = Result<Found, ScannerError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let me = self.get_mut();
        if let ScannerState::Init(_) = me.state {
            me.state.init();
        }
        if let ScannerState::Scanning(ref mut rx) = me.state {
            rx.poll_recv(cx)
        } else {
            unreachable!("broken state machine");
        }
    }
}

enum ScannerState {
    Init(PathBuf),
    Scanning(Receiver<Result<Found, ScannerError>>),
}

impl ScannerState {
    fn init(&mut self) {
        let (tx, rx) = mpsc::channel::<Result<Found, ScannerError>>(128);
        let mut state = ScannerState::Scanning(rx);
        std::mem::swap(self, &mut state);

        let s = Arc::new(Semaphore::new(32));
        tokio::task::spawn_blocking(move || do_walk(state.into_pathbuf(), tx, s));
    }

    fn into_pathbuf(self) -> PathBuf {
        match self {
            ScannerState::Init(p) => p,
            _ => panic!("Foundo pathbuf called in wrong state"),
        }
    }
}

fn do_walk(path: PathBuf, tx: Sender<Result<Found, ScannerError>>, s: Arc<Semaphore>) {
    if let Err(e) = do_walk_inner(path, tx.clone(), s) {
        let _ = tx.blocking_send(Err(e));
    }
}

fn do_walk_inner(
    path: PathBuf,
    tx: Sender<Result<Found, ScannerError>>,
    s: Arc<Semaphore>,
) -> Result<(), ScannerError> {
    let dir = walker::Walker::new(&path)?;
    for entry in dir {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".changes") || name.ends_with(".dsc") {
                let path = entry.path();
                let s = s.clone();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let _ = s.acquire().await;
                    let control = if path.extension().unwrap() == "changes" {
                        Changes::from_file(path)
                            .await
                            .map(Found::Changes)
                            .map_err(|e| e.into())
                    } else {
                        Dsc::from_file(path)
                            .await
                            .map(Found::Dsc)
                            .map_err(|e| e.into())
                    };

                    let _ = tx.send(control).await;
                });
            }
        }
    }
    Ok(())
}
