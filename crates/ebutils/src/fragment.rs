//! A fragment represents a piece of data for one event from one source of some sub-detector.
use std::fmt::{Debug, Display};

use derive_where::derive_where;

use crate::{EventId, fragment_type::FragmentType, source_id::SourceId};

/// A fragment represents a piece of data for one event from one source of some sub-detector.
///
/// Each fragment has a type, version, event id, source id and payload data.
///
/// The fragment itself keeps only a reference its data, so it can be easily copied itself.
#[derive(PartialEq, Eq)]
#[derive_where(Copy, Clone)]
pub struct Fragment<'a, Data: ?Sized + AsRef<[u8]> = [u8]> {
    r#type: u8,
    version: u8,
    event_id: EventId,
    source_id: SourceId,
    data: &'a Data,
}

impl<'a, T: ?Sized + AsRef<[u8]>> Fragment<'a, T> {
    /// Create a new fragment with the given parameters.
    ///
    /// Note that the fragment `type` is given in its raw form as `u8`.
    /// To use the typed enum, use e.g. `FragmentType::Odin as u8`.
    pub fn new(
        r#type: u8,
        version: u8,
        event_id: EventId,
        source_id: SourceId,
        data: &'a T,
    ) -> Self {
        Fragment {
            r#type,
            version,
            event_id,
            source_id,
            data,
        }
    }

    /// Returns the fragment type it its raw form as stored.
    pub fn fragment_type_raw(&self) -> u8 {
        self.r#type
    }

    /// Tries to parse the fragment type. Should an invalid or unknown type be stored, returns `None`.
    pub fn fragment_type_parsed(&self) -> Option<FragmentType> {
        FragmentType::from_repr(self.fragment_type_raw())
    }

    /// The source id where this fragment originated from.
    pub fn source_id(&self) -> SourceId {
        self.source_id
    }

    /// The event id this fragment belongs to.
    pub fn event_id(&self) -> EventId {
        self.event_id
    }

    /// The version of this fragment.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Returns a reference to the typed payload of this fragment.
    pub fn payload(&self) -> &'a T {
        self.data
    }

    /// Returns the payload of this fragment as a byte slice.
    pub fn payload_bytes(&self) -> &'a [u8] {
        self.data.as_ref()
    }

    /// Returns the size of the fragment in **bytes**, **excluding** the header.
    pub fn fragment_size(&self) -> u16 {
        size_of_val(self.data)
            .try_into()
            .expect("fragment size fits u16")
    }

    /// Transforms the fragment into another with a differently typed payload, keeping all other fields the same.
    pub fn map_payload<U: ?Sized + AsRef<[u8]>>(
        &self,
        f: impl FnOnce(&'a T) -> &'a U,
    ) -> Fragment<'a, U> {
        Fragment {
            r#type: self.r#type,
            version: self.version,
            event_id: self.event_id,
            source_id: self.source_id,
            data: f(self.data),
        }
    }
}

#[cfg(feature = "pretty")]
impl Fragment<'_> {
    pub fn pretty_print(&self, writer: &mut impl Write, indent: usize) -> std::io::Result<()> {
        use colored::Colorize;

        let config = pretty_hex::HexConfig {
            title: false,
            ascii: true,
            width: 16,
            group: 2,
            chunk: 2,
            max_bytes: 256,
            display_offset: 0,
        };

        let indent = " ".repeat(indent);
        let frag = *self;

        let name = frag
            .fragment_type_parsed()
            .map(|ty| format!("{:?}", ty))
            .unwrap_or_else(|| "Unknown".into());
        writeln!(
            writer,
            "{indent}{} {} ({:#X}) {} {}{} {}{} {} {}",
            "Fragment".bold().black(),
            name.green().bold(),
            frag.fragment_type_raw(),
            "Version".black(),
            frag.version(),
            ", Source".black(),
            frag.source_id(),
            ", Size".black(),
            frag.fragment_size(),
            "bytes".black()
        )?;

        if let Ok(odin) = frag.try_into_odin() {
            let odin = odin.payload();

            writeln!(
                writer,
                "{indent}  {:<15} {1} ({1:#X})",
                "Event Id".black(),
                odin.event_id()
            )?;
            writeln!(
                writer,
                "{indent}  {:<15} {:}",
                "Event Type".black(),
                odin.event_type()
            )?;
            writeln!(
                writer,
                "{indent}  {:<15} {:}",
                "Time".black(),
                odin.gps_time()
            )?;
            writeln!(
                writer,
                "{indent}  {:<15} {:#08X}",
                "Partition".black(),
                odin.partition_id()
            )?;
            writeln!(
                writer,
                "{indent}  {:<15} {:}",
                "Step enabled?".black(),
                odin.step_run_enable()
            )?;
            if odin.step_run_enable() {
                writeln!(
                    writer,
                    "{indent}  {:<15} {} ({1:#X})",
                    "StepNumber".black(),
                    odin.step_number()
                )?;
            }
            writeln!(
                writer,
                "{indent}  {:<15} {:?} ({1:#X})",
                "Orbit Id".black(),
                odin.orbit_id()
            )?;
            writeln!(
                writer,
                "{indent}  {:<15} {} ({1:#X})",
                "Bunch Id".black(),
                odin.bunch_id()
            )?;
            writeln!(
                writer,
                "{indent}  {:<15} {:?}",
                "BunchType".black(),
                odin.bx_type()
            )?;
            writeln!(writer, "{indent}  {:<15} {}", "TCK".black(), odin.tck())?;
            writeln!(
                writer,
                "{indent}  {:<15} {}",
                "Is nzs event?".black(),
                odin.is_nzs_event()
            )?;
            writeln!(
                writer,
                "{indent}  {:<15} {} ({1:#X})",
                "Calib type".black(),
                odin.calib_type()
            )?;
            writeln!(
                writer,
                "{indent}  {:<15} {:?} ({1:#X})",
                "Trigger type".black(),
                odin.trigger_type()
            )?;
            if odin.tae_window() > 0 {
                writeln!(
                    writer,
                    "{indent}  {:<15} {:?}",
                    "Tae window".black(),
                    odin.tae_window()
                )?;
                writeln!(
                    writer,
                    "{indent}  {:<15} {:?}",
                    "Tae central".black(),
                    odin.tae_central()
                )?;
                writeln!(
                    writer,
                    "{indent}  {:<15} {:?}",
                    "Tae first".black(),
                    odin.tae_first()
                )?;
            } else {
                writeln!(writer, "{indent}  {}", "Tae disabled".black())?;
            }
        } else {
            writeln!(
                writer,
                "{indent}  {}",
                pretty_hex::config_hex(&frag.payload_bytes(), config)
                    .replace("\n", &format!("\n  {indent}"))
            )?;
        }

        Ok(())
    }
}

impl Debug for Fragment<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data_preview = if self.data.len() > 16 {
            format!("{:02X?}... ({} bytes)", &self.data[0..16], self.data.len())
        } else {
            format!("{:02X?}", self.data)
        };

        f.debug_struct("Fragment")
            .field("type", &self.r#type)
            .field("size", &self.fragment_size())
            .field("data", &data_preview)
            .field("version", &self.version)
            .field("event_id", &self.event_id)
            .field("source_id", &self.source_id)
            .finish()
    }
}

impl Display for Fragment<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Fragment[type={}, size={}]",
            self.r#type,
            self.fragment_size()
        )
    }
}
