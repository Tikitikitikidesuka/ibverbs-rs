use std::fmt::{Debug, Display};

use strum::FromRepr;

/// This enum represents all the known fragment types.
///
/// They are documented [here](https://gitlab.cern.ch/lhcb/Online/-/blob/master/Online/EventData/include/EventData/bank_types_t.h).
///
/// Additionally, some special error marker types are included.
#[repr(u8)]
#[derive(Copy, Clone, Debug, FromRepr, PartialEq, Eq)]
pub enum FragmentType {
    L0Calo = 0,             //  0
    L0DU,                   //  1
    PrsE,                   //  2
    EcalE,                  //  3
    HcalE,                  //  4
    PrsTrig,                //  5
    EcalTrig,               //  6
    HcalTrig,               //  7
    Velo,                   //  8
    Rich,                   //  9
    TT = 10,                // 10
    IT,                     // 11
    OT,                     // 12
    Muon,                   // 13
    L0PU,                   // 14
    DAQ,                    // 15
    ODIN,                   // 16
    HltDecReports,          // 17
    VeloFull,               // 18
    TTFull,                 // 19
    ITFull = 20,            // 20
    EcalPacked,             // 21
    HcalPacked,             // 22
    PrsPacked,              // 23
    L0Muon,                 // 24
    ITError,                // 25
    TTError,                // 26
    ITPedestal,             // 27
    TTPedestal,             // 28
    VeloError,              // 29
    VeloPedestal = 30,      // 30
    VeloProcFull,           // 31
    OTRaw,                  // 32
    OTError,                // 33
    EcalPackedError,        // 34
    HcalPackedError,        // 35
    PrsPackedError,         // 36
    L0CaloFull,             // 37
    L0CaloError,            // 38
    L0MuonCtrlAll,          // 39
    L0MuonProcCand = 40,    // 40
    L0MuonProcData,         // 41
    L0MuonRaw,              // 42
    L0MuonError,            // 43
    GaudiSerialize,         // 44
    GaudiHeader,            // 45
    TTProcFull,             // 46
    ITProcFull,             // 47
    TAEHeader,              // 48
    MuonFull,               // 49
    MuonError = 50,         // 50
    TestDet,                // 51
    L0DUError,              // 52
    HltRoutingBits,         // 53
    HltSelReports,          // 54
    HltVertexReports,       // 55
    HltLumiSummary,         // 56
    L0PUFull,               // 57
    L0PUError,              // 58
    DstBank,                // 59
    DstData = 60,           // 60
    DstAddress,             // 61
    FileID,                 // 62
    VP,                     // 63  Does not exist in hardware. Only SIM
    FTCluster,              // 64
    VL,                     // 65
    UT,                     // 66
    UTFull,                 // 67
    UTError,                // 68
    UTPedestal,             // 69
    HC = 70,                // 70
    HltTrackReports,        // 71
    HCError,                // 72
    VPRetinaCluster,        // 73
    FTGeneric,              // 74
    FTCalibration,          // 75
    FTNZS,                  // 76
    Calo,                   // 77
    CaloError,              // 78
    MuonSpecial,            // 79
    RichCommissioning = 80, // 80
    RichError,              // 81
    FTSpecial,              // 82
    CaloSpecial,            // 83
    Plume,                  // 84
    PlumeSpecial,           // 85
    PlumeError,             // 86
    VeloThresholdScan,      // 87  Hardware only ?
    FTError,                // 88

    /* Banks above are reserved for DAQ, add only generic DaqError types below. */
    DaqErrorFragmentThrottled = 89, //
    DaqErrorBXIDCorrupted = 90,     //
    DaqErrorSyncBXIDCorrupted = 91, //
    DaqErrorFragmentMissing = 92,   //
    DaqErrorFragmentTruncated = 93, //
    DaqErrorIdleBXIDCorrupted = 94, //
    DaqErrorFragmentMalformed = 95, //
    DaqErrorEVIDJumped = 96,        //

    /* Banks below again for DAQ */
    VeloSPPandCluster = 97,        //
    UTNZS = 98,                    //
    UTSpecial = 99,                //
    DaqErrorAlignFifoFull = 100,   // 100
    DaqErrorFEfragSizeWrong = 101, // 101

    /// Not defined with the others, from [here](https://gitlab.cern.ch/lhcb-daq40/lhcb-daq40-software/-/blob/master/common/rawbanktype.h).
    PcieTest = 254,
}

impl Display for FragmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}({})", *self as u8)
    }
}
