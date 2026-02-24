use crate::ibverbs::device::Device;
use std::io;

impl<'a> Device<'a> {
    /// Bind the calling task (OS thread) to the NUMA node local to this InfiniBand device.
    ///
    /// This reads the device’s NUMA node from sysfs (`/sys/class/infiniband/<dev>/device/numa_node`)
    /// and then applies the affinity using `numa_run_on_node()`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The device name is not available (`self.name()` is `None`).
    /// - The sysfs file cannot be read (I/O error).
    /// - The sysfs contents cannot be parsed as an `i32` (reported as `InvalidData`).
    /// - `numa_run_on_node()` fails (returns `-1` and sets `errno`; returned via
    ///   [`io::Error::last_os_error`]).
    pub fn bind_thread_to_numa(&self) -> io::Result<()> {
        let dev = self
            .name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid device name"))?;

        let numa = get_numa_node(dev)?;

        set_numa_node(numa)?;

        log::debug!("Task bound to numa node {numa}");
        Ok(())
    }

    pub fn bind_thread_to_numa_strict(&self) -> io::Result<()> {
        let dev = self
            .name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid device name"))?;

        let numa = get_numa_node(dev)?;

        set_numa_node_strict(numa)?;

        log::debug!("Task bound to numa node {numa}");
        Ok(())
    }
}

/// Pins the current task (OS thread) to the specified NUMA node.
///
/// This is a thin wrapper around `numa_run_on_node()`. On success, it returns `Ok(())`; on failure
/// it returns the OS error reported via `errno`.
///
/// Passing `-1` to `numa_run_on_node()` permits the kernel to schedule the task on all nodes again,
/// effectively resetting the affinity.
fn set_numa_node(node: i32) -> io::Result<()> {
    let res = unsafe { numa_run_on_node(node) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }

    // Allocate future memory from this node
    unsafe { numa_set_localalloc() };

    Ok(())
}

/// Pins the current task (OS thread) to the specified NUMA node.
///
/// This is a thin wrapper around `numa_run_on_node()`. On success, it returns `Ok(())`; on failure
/// it returns the OS error reported via `errno`.
///
/// Passing `-1` to `numa_run_on_node()` permits the kernel to schedule the task on all nodes again,
/// effectively resetting the affinity.
fn set_numa_node_strict(node: i32) -> io::Result<()> {
    let res = unsafe { numa_run_on_node(node) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }

    // 1 = strict binding (no fallback to other nodes)
    unsafe { numa_set_bind_policy(1) };
    // Allocate future memory from this node
    unsafe { numa_set_localalloc() };

    Ok(())
}

unsafe extern "C" {
    fn numa_run_on_node(node: std::os::raw::c_int) -> std::os::raw::c_int;
    fn numa_set_localalloc();
    fn numa_set_bind_policy(strict: std::os::raw::c_int);
}

/// Read the NUMA node for an InfiniBand device from sysfs.
///
/// Reads `/sys/class/infiniband/<dev>/device/numa_node` and parses it as an `i32`.
///
/// # Errors
///
/// Returns an error if the file cannot be read, or if the contents cannot be parsed as an `i32`.
fn get_numa_node(dev: &str) -> io::Result<i32> {
    let numa_path = format!("/sys/class/infiniband/{dev}/device/numa_node");
    let s = std::fs::read_to_string(numa_path)?;
    let node = s
        .trim()
        .parse::<i32>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    if node < 0 {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("numa node for {dev} not found"),
        ))
    } else {
        Ok(node)
    }
}
