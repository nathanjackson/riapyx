extern crate ansi_term;
#[macro_use]
mod color_print;

mod cpu;
mod bios;
mod mem;
mod machine;
mod hw;

use std::io;
use std::io::prelude::*;
use std::string::String;
use std::collections::HashMap;
use std::env::args;
use std::{thread, time};
use std::sync::mpsc;
use std::sync::mpsc::{SyncSender, Receiver};
use bios::BootDrive;

extern crate rustc_serialize;
extern crate docopt;
use docopt::Docopt;

extern crate num_traits;
extern crate iced_x86;

const USAGE: &'static str = 
"Usage:
	riapyx [--boot=<drive>] [--hd=<image>] [--fd=<image>]
	riapyx [--help]

Options:
	--help           Display this message
	--hd=<img>       Hard drive image
	--fd=<img>       Floppy disk image
	--boot=<drive>   Boot from floppy disk (fd) or hard drive (hd) [default: hd]
";

#[derive(Debug, RustcDecodable)]
struct Args
{
	flag_hd: Option<String>,
	flag_fd: Option<String>,
	flag_boot: String
}

const ZERO_U8: u8 = '0' as u8;
const NINE_U8: u8 = '9' as u8;
const A_U8: u8 = 'a' as u8;
const F_U8: u8 = 'f' as u8;
fn u32_from_hex_str(s: &str) -> u32
{
	let mut res: u32 = 0;

	for c in s.as_bytes()
	{
		res *= 0x10;
		
		let to_add = 
			match *c
			{
				ZERO_U8 ... NINE_U8 => c - ZERO_U8,
				A_U8 ... F_U8 => (c - A_U8) + 0xa,
				_ => panic!("Invalid hex string: {}", s)
			};

		res += to_add as u32;
	}

	res
}

trait Command
{
    fn execute(&self, m: &mut machine::Machine);
}

struct QuitCommand { }

impl Command for QuitCommand
{
    fn execute(&self, m: &mut machine::Machine)
    {
        std::process::exit(0);
    }
}

struct DumpCommand
{
    seg: u16,
    addr: u16
}

impl Command for DumpCommand
{
    fn execute(&self, m: &mut machine::Machine)
    {
        m.print_memory(self.seg, self.addr, 16);
    }
}

struct ContinueCommand
{
    trace: bool
}

impl Command for ContinueCommand
{
    fn execute(&self, m: &mut machine::Machine)
    {
        m.resume(self.trace);
    }
}

struct TraceCommand { }

impl Command for TraceCommand
{
    fn execute(&self, m: &mut machine::Machine)
    {
        println!("not yet implemented");
    }
}

//type Command = Box<impl Fn() + Send>;

//fn do_quit_cmd()
//{
//    std::process::exit(0);
//}

//fn do_disas_cmd(seg: u16, addr: u16)
//{
    //m.print_memory(seg, addr, 16);

				/*let args = (words.next(), words.next());
				match args
				{
					(Some(seg_str), Some(addr_str)) =>
					{
						let seg = u32_from_hex_str(seg_str) as u16;
						let addr = u32_from_hex_str(addr_str) as u16;

						m.print_memory(seg, addr, 16);
					},
					_ => debug_print!("Usage: d [segment] [address]")
				}
			}*/
//}


fn console_thread(tx: SyncSender<Box<dyn Command + Send>>)
{
    loop {
        print!("debug> ");
        io::stdout().flush().unwrap();
        let mut cmd_str = String::new();
        io::stdin().read_line(&mut cmd_str).unwrap();

		let mut words = cmd_str.split_whitespace();

        let cmd: Box<dyn Command + Send>;
		let word0 = words.next();
		match word0
		{
            Some("q") => { 
                cmd = Box::new(QuitCommand{ });
            }
            Some("d") => {
				let args = (words.next(), words.next());
				match args
				{
					(Some(seg_str), Some(addr_str)) =>
					{
						let seg = u32_from_hex_str(seg_str) as u16;
						let addr = u32_from_hex_str(addr_str) as u16;

                        cmd = Box::new(DumpCommand{
                            seg,
                            addr
                        });
					},
					_ => {
                        debug_print!("Usage: d [segment] [address]");
                        continue
                    }
                }
			}
            Some("c") | Some("t") => {
                let trace = Some("t") == word0;
                cmd = Box::new(ContinueCommand{
                    trace
                });
            }
            _ => {
                println!("Bad command.");
                continue
            }
        }

        tx.send(cmd);
    }
}

fn main()
{
	let args: Args = Docopt::new(USAGE).and_then(|d| d.decode()).unwrap_or_else(|e| e.exit());

	let boot_drive = 
		match &args.flag_boot[..]
		{
			"fd" => BootDrive::Floppy,
			"hd" => BootDrive::HardDrive,
			_ => panic!("Unrecognized boot drive: '{}'. use 'hd' or 'fd'", args.flag_boot)
		};

	match boot_drive
	{
		BootDrive::Floppy =>
		{
			if let None = args.flag_fd
			{ panic!("Booting from floppy disk, but no floppy image specified"); }
		},
		BootDrive::HardDrive =>
		{
			if let None = args.flag_hd 
			{ panic!("Booting from hard drive, but no hard drive image specified"); }
		}
	}

	let mut m = machine::Machine::new(
				boot_drive,
				args.flag_fd,
				args.flag_hd);
	m.dump();

    // channel to communicate console commands to the emulator loop
    let (tx, rx): (SyncSender<Box<dyn Command + Send>>, Receiver<Box<dyn Command + Send>>) = mpsc::sync_channel(1);

    // console thread
    let console_thread_handle = thread::spawn(move || {
        console_thread(tx);
    });

    // main emulator loop
    loop {
        match rx.try_recv() {
            Ok(cmd) => {
                (*cmd).execute(&mut m);
            }
            Err(_) => {
                m.step();
            }
        }
    }

    console_thread_handle.join().unwrap();

	/*let mut bkpt_addr: HashMap<i32, (u16, u16)> = HashMap::new();
	let mut addr_bkpt: HashMap<(u16, u16), i32> = HashMap::new();
	let mut next_bkpt_idx = 0;

	loop
	{
		print!("debug> ");
		io::stdout().flush().unwrap();
		let mut cmd = String::new();
		io::stdin().read_line(&mut cmd).unwrap();

        m.hw.sdl.event().unwrap().flush_events(0x0, 0xffffffff);

		let mut words = cmd.split_whitespace();

		let cmd = words.next();
		match cmd
		{
			Some("d") => 
			{
				let args = (words.next(), words.next());
				match args
				{
					(Some(seg_str), Some(addr_str)) =>
					{
						let seg = u32_from_hex_str(seg_str) as u16;
						let addr = u32_from_hex_str(addr_str) as u16;

						m.print_memory(seg, addr, 16);
					},
					_ => debug_print!("Usage: d [segment] [address]")
				}
			},
			Some("u") => 
			{
				let args = (words.next(), words.next());
				match args
				{
					(Some(cs_str), Some(ip_str)) =>
					{
						let cs = u32_from_hex_str(cs_str) as u16;
						let ip = u32_from_hex_str(ip_str) as u16;

						m.disas(cs, ip, 5);
					},
					_ => debug_print!("Usage: u [segment] [address]")
				}
			},
			Some("b") =>
			{
				let args = (words.next(), words.next());
				match args
				{
					(Some(cs_str), Some(ip_str)) =>
					{
						let cs = u32_from_hex_str(cs_str) as u16;
						let ip = u32_from_hex_str(ip_str) as u16;

						bkpt_addr.insert(next_bkpt_idx, (cs, ip));
						addr_bkpt.insert((cs, ip), next_bkpt_idx);
						debug_print!("Breakpoint {} set at {:04x}:{:04x}", next_bkpt_idx, cs, ip);
						next_bkpt_idx += 1;
					},
					_ => debug_print!("Usage: b [segment] [address]")
				}
			}
			Some("w") =>
			{
				let arg = words.next();
				match arg
				{
					Some(fname) =>
					{
						m.dump_memory_to_file(fname);
					},
					_ => debug_print!("Usage: w [filename]")
				}
			}
			Some("ws") =>
			{
				let args = (words.next(), words.next());
				match args
				{
					(Some(seg), Some(fname)) =>
					{
						m.dump_segment_to_file(u32_from_hex_str(seg), fname);
					},
					_ => debug_print!("Usage: ws [seg] [filename]")
				}
			}
			Some("c") | Some("t") =>
			{
				loop
				{
					m.step();

					if cmd == Some("t")
					{
						m.dump_trace();
					}

					if addr_bkpt.contains_key(&m.get_pc())
					{
						let (cs, ip) = m.get_pc();
						let idx = addr_bkpt.get(&(cs, ip)).expect("Broke with no breakpoint?");
						debug_print!("Hit breakpoint #{} at {:04x}:{:04x}", idx, cs, ip);
						break;
					}
					if ! m.is_running()
					{
						debug_print!("Machine halted");
						break;
					}
				}

				m.dump();				
			}
			Some("f") =>
			{
				while m.is_running()
				{
					m.step();
				}
				m.dump();
			}
			Some("q") => std::process::exit(0),
			Some(s) =>
			{
				debug_print!("Unknown command: {}", s)
			}
			None =>
			{
				m.step();
				m.dump();
			}
		}
	}*/
}
