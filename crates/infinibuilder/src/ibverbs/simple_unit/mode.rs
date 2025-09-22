use crate::connection::Connect;

pub trait Mode {
    type UnconnectedMr: Connect<ConnectionConfig = Self::MrConnectionConfig, Connected = Self::ConnectedMr>;
    type ConnectedMr;
    type MrConnectionConfig;
}
