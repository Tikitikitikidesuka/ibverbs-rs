use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IbBUncheckedStaticNetworkConfig {
    pub(crate) node_config_vec: Vec<IbBStaticNodeConfig>,
}

#[derive(Debug, Clone)]
pub struct IbBCheckedStaticNetworkConfig {
    node_config_map: HashMap<u32, IbBStaticNodeConfig>,
    rank_ids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct IbBStaticNodeConfig {
    hostname: String,
    ib_device: String,
    rank_id: u32,
    ut_id: String,
    role: IbBNodeRole,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum IbBNodeRole {
    ReadoutUnit,
    BuilderUnit,
}

impl Deref for IbBUncheckedStaticNetworkConfig {
    type Target = [IbBStaticNodeConfig];

    fn deref(&self) -> &Self::Target {
        self.node_config_vec.as_slice()
    }
}

impl Deref for IbBCheckedStaticNetworkConfig {
    type Target = HashMap<u32, IbBStaticNodeConfig>;

    fn deref(&self) -> &Self::Target {
        &self.node_config_map
    }
}

impl IbBCheckedStaticNetworkConfig {
    pub fn iter(&self) -> IbBStaticNetworkIter {
        IbBStaticNetworkIter {
            rank_ids: &self.rank_ids,
            node_map: &self.node_config_map,
            index: 0,
        }
    }
}

pub struct IbBStaticNetworkIter<'a> {
    rank_ids: &'a Vec<u32>,
    node_map: &'a HashMap<u32, IbBStaticNodeConfig>,
    index: usize,
}

impl<'a> Iterator for IbBStaticNetworkIter<'a> {
    type Item = &'a IbBStaticNodeConfig;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.node_map.get(self.rank_ids.get(self.index)?);
        self.index += 1;
        res
    }
}

impl Serialize for IbBCheckedStaticNetworkConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert self to an Unchecked equivalent and serialize that
        let unchecked = IbBUncheckedStaticNetworkConfig {
            node_config_vec: self
                .node_config_map
                .values()
                .cloned()
                .collect::<Vec<IbBStaticNodeConfig>>(),
        };
        unchecked.serialize(serializer)
    }
}

impl IbBUncheckedStaticNetworkConfig {
    pub fn new() -> Self {
        Self {
            node_config_vec: Vec::new(),
        }
    }

    pub fn add_node(mut self, node_config: IbBStaticNodeConfig) -> Self {
        self.node_config_vec.push(node_config);
        self
    }

    pub fn validate(self) -> Result<IbBCheckedStaticNetworkConfig, (Self, String)> {
        let mut rank_ids = Vec::new();
        let mut seen = HashSet::new();
        let mut node_config_map = HashMap::new();

        for node in &self.node_config_vec {
            let rank_id = node.rank_id;

            if !seen.insert(rank_id) {
                return Err((self, format!("Duplicate rank_id found: {}", rank_id)));
            }

            node_config_map.insert(rank_id, node.clone());
            rank_ids.push(rank_id);
        }

        Ok(IbBCheckedStaticNetworkConfig {
            node_config_map,
            rank_ids,
        })
    }
}

impl FromIterator<IbBStaticNodeConfig> for IbBUncheckedStaticNetworkConfig {
    fn from_iter<T: IntoIterator<Item = IbBStaticNodeConfig>>(iter: T) -> Self {
        Self {
            node_config_vec: iter.into_iter().collect(),
        }
    }
}

impl IbBStaticNodeConfig {
    pub fn new<S: Into<String>>(hostname: S, ib_device: S, rank_id: u32, ut_id: S, role: IbBNodeRole) -> Self {
        Self {
            hostname: hostname.into(),
            ib_device: ib_device.into(),
            rank_id,
            ut_id: ut_id.into(),
            role
        }
    }

    pub fn hostname(&self) -> &str {
        self.hostname.as_str()
    }

    pub fn ib_device(&self) -> &str {
        self.ib_device.as_str()
    }

    pub fn rank_id(&self) -> u32 {
        self.rank_id
    }

    pub fn ut_id(&self) -> &str {
        self.ut_id.as_str()
    }

    pub fn role(&self) -> IbBNodeRole {
        self.role
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use crate::IbBNodeRole::ReadoutUnit;

    fn make_node(rank_id: u32) -> IbBStaticNodeConfig {
        IbBStaticNodeConfig {
            hostname: format!("host{}", rank_id),
            ib_device: format!("device{}", rank_id),
            rank_id,
            ut_id: format!("ut{}", rank_id),
            role: ReadoutUnit,
        }
    }

    #[test]
    fn test_add_and_validate_ok() {
        let config = IbBUncheckedStaticNetworkConfig::new()
            .add_node(make_node(1))
            .add_node(make_node(2));

        let checked = config.validate();
        assert!(checked.is_ok());

        let checked = checked.unwrap();
        assert_eq!(checked.node_config_map.len(), 2);
        assert!(checked.node_config_map.contains_key(&1));
        assert!(checked.node_config_map.contains_key(&2));
    }

    #[test]
    fn test_validate_duplicate_rank_id() {
        let config = IbBUncheckedStaticNetworkConfig::new()
            .add_node(make_node(1))
            .add_node(make_node(1)); // duplicate rank_id

        let result = config.validate();
        assert!(result.is_err());

        let (original, err_msg) = result.unwrap_err();
        assert!(err_msg.contains("Duplicate rank_id"));
        assert_eq!(original.node_config_vec.len(), 2); // Original config preserved
    }

    #[test]
    fn test_serde_unchecked_roundtrip() {
        let json = r#"
        {
            "node_config_vec": [
                {"hostname":"host1","ib_device":"device1","rank_id":1,"ut_id":"ut1"},
                {"hostname":"host2","ib_device":"device2","rank_id":2,"ut_id":"ut2"}
            ]
        }
        "#;

        let config: IbBUncheckedStaticNetworkConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.node_config_vec.len(), 2);

        let serialized = serde_json::to_string(&config).unwrap();
        assert!(serialized.contains("host1"));
        assert!(serialized.contains("device2"));
    }

    #[test]
    fn test_serialize_checked_matches_unchecked_format() {
        let unchecked =
            IbBUncheckedStaticNetworkConfig::from_iter(vec![make_node(1), make_node(2)]);
        let checked = unchecked.clone().validate().unwrap();

        let unchecked_json = serde_json::to_string(&unchecked).unwrap();
        let checked_json = serde_json::to_string(&checked).unwrap();

        // Both should serialize identically
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&unchecked_json).unwrap(),
            serde_json::from_str::<serde_json::Value>(&checked_json).unwrap()
        );
    }

    #[test]
    fn test_serialize_checked_contains_all_nodes() {
        let checked =
            IbBUncheckedStaticNetworkConfig::from_iter(vec![make_node(42), make_node(13)])
                .validate()
                .unwrap();

        let json = serde_json::to_string(&checked).unwrap();
        assert!(json.contains("host42"));
        assert!(json.contains("device13"));
        assert!(json.contains("ut42"));
    }
}
