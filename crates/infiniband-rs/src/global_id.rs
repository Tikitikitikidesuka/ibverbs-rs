use ibverbs_sys::*;

/// A Global identifier for ibv.
///
/// This struct acts as a rust wrapper for `ibv_gid`. We use it instead of
/// `ibv_giv` because `ibv_gid` is actually an untagged union.
///
/// ```c
/// union ibv_gid {
///     uint8_t   raw[16];
///     struct {
///         __be64 subnet_prefix;
///         __be64 interface_id;
///     } global;
/// };
/// ```
///
/// It appears that `global` exists for convenience, but can be safely ignored.
/// For continuity, the methods `subnet_prefix` and `interface_id` are provided.
/// These methods read the array as big endian, regardless of native cpu
/// endianness.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct IbvGid {
    raw: [u8; 16],
}

impl IbvGid {
    /// Expose the subnet_prefix component of the `Gid` as a u64. This is
    /// equivalent to accessing the `global.subnet_prefix` component of the
    /// `ibv_gid` union.
    pub fn subnet_prefix(&self) -> u64 {
        u64::from_be_bytes(self.raw[..8].try_into().unwrap())
    }

    /// Expose the interface_id component of the `Gid` as a u64. This is
    /// equivalent to accessing the `global.interface_id` component of the
    /// `ibv_gid` union.
    pub fn interface_id(&self) -> u64 {
        u64::from_be_bytes(self.raw[8..].try_into().unwrap())
    }
}

impl From<ibv_gid> for IbvGid {
    fn from(gid: ibv_gid) -> Self {
        Self {
            raw: unsafe { gid.raw },
        }
    }
}

impl From<IbvGid> for ibv_gid {
    fn from(mut gid: IbvGid) -> Self {
        *gid.as_mut()
    }
}

impl From<IbvGid> for [u8; 16] {
    fn from(gid: IbvGid) -> Self {
        gid.raw
    }
}

impl From<[u8; 16]> for IbvGid {
    fn from(raw: [u8; 16]) -> Self {
        Self { raw }
    }
}

impl AsRef<ibv_gid> for IbvGid {
    fn as_ref(&self) -> &ibv_gid {
        unsafe { &*self.raw.as_ptr().cast::<ibv_gid>() }
    }
}

impl AsMut<ibv_gid> for IbvGid {
    fn as_mut(&mut self) -> &mut ibv_gid {
        unsafe { &mut *self.raw.as_mut_ptr().cast::<ibv_gid>() }
    }
}

/// A Global identifier entry for ibv.
///
/// This struct acts as a rust wrapper for `ibv_gid_entry`. We use it instead of
/// `ibv_gid_entry` because `ibv_gid` is wrapped by `Gid`.
#[derive(Debug, Clone)]
pub struct IbvGidEntry {
    /// The GID entry.
    pub gid: IbvGid,
    /// The GID table index of this entry.
    pub gid_index: u32,
    /// The port number that this GID belongs to.
    pub port_num: u32,
    /// enum ibv_gid_type, can be one of IBV_GID_TYPE_IB, IBV_GID_TYPE_ROCE_V1 or IBV_GID_TYPE_ROCE_V2.
    pub gid_type: ibv_gid_type,
    /// The interface index of the net device associated with this GID.
    ///
    /// It is 0 if there is no net device associated with it.
    pub ndev_ifindex: u32,
}

impl From<ibv_gid_entry> for IbvGidEntry {
    fn from(gid_entry: ibv_gid_entry) -> Self {
        Self {
            gid: gid_entry.gid.into(),
            gid_index: gid_entry.gid_index,
            port_num: gid_entry.port_num,
            gid_type: match gid_entry.gid_type {
                0 => IBV_GID_TYPE_IB,
                1 => IBV_GID_TYPE_ROCE_V1,
                2 => IBV_GID_TYPE_ROCE_V2,
                x => panic!("unknown ibv_gid_type: {x}"),
            },
            ndev_ifindex: gid_entry.ndev_ifindex,
        }
    }
}
