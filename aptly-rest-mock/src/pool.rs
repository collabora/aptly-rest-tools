use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Package {
    fields: serde_json::Value,
}

impl Package {
    pub fn fields(&self) -> &serde_json::Value {
        &self.fields
    }
}

#[derive(Debug)]
pub struct Pool {
    packages: HashMap<String, Package>,
}

impl Pool {
    pub fn new() -> Self {
        Pool {
            packages: HashMap::new(),
        }
    }

    pub(crate) fn add_json_package(&mut self, fields: serde_json::Value) {
        let key = fields["Key"]
            .as_str()
            .expect("Missing key in package fields")
            .to_string();
        let p = Package { fields };
        self.packages.insert(key, p);
    }

    pub fn package(&self, key: &str) -> Option<&Package> {
        self.packages.get(key)
    }

    pub fn has_package(&self, key: &str) -> bool {
        self.package(key).is_some()
    }
}
