#[derive(Debug)]
pub struct BinaryPackage {
    pub source: String,
}

#[derive(Debug)]
pub struct SourcePackage {
    pub source: String,
}

#[derive(Debug)]
pub enum Package {
    BinaryPackage(BinaryPackage),
    SourcePackage(SourcePackage),
}
