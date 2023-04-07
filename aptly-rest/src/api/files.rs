use reqwest::Url;
use tokio::io::AsyncRead;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::AptlyRestError;

pub struct UploadFiles {
    parts: Vec<reqwest::multipart::Part>,
}

impl UploadFiles {
    pub fn new() -> Self {
        Self { parts: vec![] }
    }

    pub fn add_file(&mut self, filename: String, contents: impl AsyncRead + Send + Sync + 'static) {
        let body = reqwest::Body::wrap_stream(FramedRead::new(contents, BytesCodec::new()));
        self.parts
            .push(reqwest::multipart::Part::stream(body).file_name(filename));
    }

    pub fn file(
        mut self,
        filename: String,
        contents: impl AsyncRead + Send + Sync + 'static,
    ) -> Self {
        self.add_file(filename, contents);
        self
    }
}

#[derive(Debug, Clone)]
pub struct FilesApi<'a> {
    pub(crate) aptly: &'a crate::AptlyRest,
}

impl FilesApi<'_> {
    pub fn directory(&self, directory: String) -> FilesApiDirectory {
        FilesApiDirectory {
            files: self,
            directory,
        }
    }

    pub async fn list_directories(&self) -> Result<Vec<String>, AptlyRestError> {
        self.aptly.get(self.aptly.url(&["api", "files"])).await
    }
}

#[derive(Debug, Clone)]
pub struct FilesApiDirectory<'a> {
    files: &'a FilesApi<'a>,
    directory: String,
}

impl FilesApiDirectory<'_> {
    fn url(&self) -> Url {
        self.files.aptly.url(&["api", "files", &self.directory])
    }

    pub async fn list(&self) -> Result<Vec<String>, AptlyRestError> {
        self.files.aptly.get(self.url()).await
    }

    pub async fn delete(&self) -> Result<(), AptlyRestError> {
        let req = self.files.aptly.client.delete(self.url());
        self.files.aptly.send_request(req).await?;
        Ok(())
    }

    pub async fn upload(&self, upload: UploadFiles) -> Result<(), AptlyRestError> {
        let form = upload
            .parts
            .into_iter()
            .fold(reqwest::multipart::Form::new(), |form, part| {
                form.part("file", part)
            });
        let req = self.files.aptly.client.post(self.url()).multipart(form);
        self.files.aptly.send_request(req).await?;

        Ok(())
    }

    pub fn file(&self, filename: String) -> FilesApiDirectoryFile<'_> {
        FilesApiDirectoryFile {
            directory: self,
            filename,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FilesApiDirectoryFile<'a> {
    directory: &'a FilesApiDirectory<'a>,
    filename: String,
}

impl FilesApiDirectoryFile<'_> {
    fn url(&self) -> Url {
        self.directory
            .files
            .aptly
            .url(&["api", "files", &self.directory.directory, &self.filename])
    }

    pub async fn delete(&self) -> Result<(), AptlyRestError> {
        let req = self.directory.files.aptly.client.delete(self.url());
        self.directory.files.aptly.send_request(req).await?;
        Ok(())
    }
}
