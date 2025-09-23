use crate::connect::Connect;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub trait Mode {
    type UnconnectedMr: Connect<ConnectionConfig = Self::MrConnectionConfig, Connected = Self::ConnectedMr>;
    type ConnectedMr;
    type MrConnectionConfig;//: Serialize + DeserializeOwned;
}
