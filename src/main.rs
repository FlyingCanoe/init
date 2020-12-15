//#![deny(warnings)]
#![feature(dbg_macro)]
#![feature(duration_float)]

mod command;
mod dep_graph;
mod legacy;
mod service;
mod service_tree;

use std::env;
use std::fs::{self, File};
use std::io::{Error, Result};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::time::Instant;

use colored::{ColoredString, Colorize};
use fern::Dispatch;
use log::{error, Level, LevelFilter};
use syscall::flag::{O_RDONLY, O_WRONLY, WaitFlags, CloneFlags};

use crate::service::Service;
use crate::service_tree::ServiceGraph;

const INITFS_SERVICE_DIR: &str = "initfs:/etc/init.d";
const FS_SERVICE_DIR: &str = "file:/etc/init.d";

fn switch_stdio(stdio: &str) -> Result<()> {
    let stdin = unsafe { File::from_raw_fd(
        syscall::open(stdio, O_RDONLY).map_err(|err| Error::from_raw_os_error(err.errno))? as RawFd
    ) };
    let stdout = unsafe { File::from_raw_fd(
        syscall::open(stdio, O_WRONLY).map_err(|err| Error::from_raw_os_error(err.errno))? as RawFd
    ) };
    let stderr = unsafe { File::from_raw_fd(
        syscall::open(stdio, O_WRONLY).map_err(|err| Error::from_raw_os_error(err.errno))? as RawFd
    ) };
    
    syscall::dup2(stdin.as_raw_fd() as usize, 0, &[]).map_err(|err| Error::from_raw_os_error(err.errno))?;
    syscall::dup2(stdout.as_raw_fd() as usize, 1, &[]).map_err(|err| Error::from_raw_os_error(err.errno))?;
    syscall::dup2(stderr.as_raw_fd() as usize, 2, &[]).map_err(|err| Error::from_raw_os_error(err.errno))?;
    
    Ok(())
}

trait PathExt {
    fn scheme(&self) -> Option<PathBuf>;
}

impl PathExt for Path {
    // Credit to @stratact for this implemenation
    fn scheme(&self) -> Option<PathBuf> {
        let path = self.as_os_str()
            .to_string_lossy();
        
        path.find(':')
            .map(|i| path[..i + 1].into())
    }
}

pub fn main() {
    let start_time = Instant::now();
    //env::set_var("RUST_BACKTRACE", "1");
    
    // Could use fern for this, but standard length...
    fn color(lvl: Level) -> ColoredString {
        match lvl {
            Level::Error => "Error".red(),
            Level::Warn  => "Warn ".yellow(),
            Level::Info  => "Info ".blue(),
            Level::Debug => "Debug".green(),
            Level::Trace => "Trace".into()
        }.bold()
    }
    
    Dispatch::new()
        .format(move |out, message, record| {
            let time = Instant::now()
                .duration_since(start_time);
            let time = format!("{:.3}", time.as_secs());
            
            out.finish(format_args!(
                "[ {} ][ {} ] {}",
                time.green(),
                color(record.level()),
                message
            ))
        })
        .chain(std::io::stdout())
        .level(LevelFilter::Trace)
        .apply()
        .unwrap_or_else(|err| {
            println!("init: failed to start logger: {}", err);
        });
    
    // This way we can continue to support old systems that still have init.rc
    if let Ok(_) = fs::metadata("initfs:/etc/init.rc") {
        if let Err(err) = legacy::run(&Path::new("initfs:/etc/init.rc")) {
            error!("failed to run initfs:/etc/init.rc: {}", err);
        }
    } else {
        let service_graph = ServiceGraph::new();
        
        let initfs_services = Service::from_dir(INITFS_SERVICE_DIR)
            .unwrap_or_else(|err| {
                error!("failed to parse service directory '{}': {}", INITFS_SERVICE_DIR, err);
                vec![]
            });
        
        service_graph.push_services(initfs_services);
        service_graph.start_services();
        
        /* Helpful to disable for debugging
        crate::switch_stdio("display:1")
            .unwrap_or_else(|err| {
                error!("error switching stdio: {}", err);
            });
        // */
        
        //* Needed for redox_users in order to parse the right passwd/group files
        env::set_current_dir("file:")
            .unwrap_or_else(|err| {
                error!("failed to set cwd: {}", err);
            });
        // */
        
        let fs_services = Service::from_dir(FS_SERVICE_DIR)
            .unwrap_or_else(|err| {
                error!("failed to parse service directory '{}': {}", FS_SERVICE_DIR, err);
                vec![]
            });
        
        service_graph.push_services(fs_services);
        service_graph.start_services();
    }
    
    // Might should not do this
    syscall::setrens(0, 0).expect("init: failed to enter null namespace");
    
    loop {
        let mut status = 0;
        syscall::waitpid(0, &mut status, WaitFlags::empty()).unwrap();
    }
}
