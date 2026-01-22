use phase::PSDebugger;

pub fn debug_mode(mut debugger: PSDebugger) {
    println!("Debug mode.");
    println!("Enter 'h' for help.");

    let mut breaks = std::collections::BTreeSet::new();
    let mut stack_trace = Vec::new();
    loop {
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => if input.starts_with("b:") {
                // Add breakpoint
                match u32::from_str_radix(&input[2..].trim(), 16) {
                    Ok(num) => {
                        println!("Inserted breakpoint at ${:08X}", num);
                        breaks.insert(num);
                    },
                    Err(e) => println!("Invalid breakpoint: {}", e),
                }
            } else if input.starts_with("c:") {
                // Remove breakpoint
                match u32::from_str_radix(&input[2..].trim(), 16) {
                    Ok(num) => {
                        println!("Cleared breakpoint at ${:08X}", num);
                        breaks.remove(&num);
                    },
                    Err(e) => println!("Invalid breakpoint: {}", e),
                }
            } else if input.starts_with("c") {
                // Remove all breakpoints
                println!("Cleared all breakpoints");
                breaks.clear();
            } else if input.starts_with("r") {
                // Run
                loop {
                    let state = debugger.get_state();
                    let loc = state.pc;
                    if breaks.contains(&loc) {
                        println!("Break at ${:08X}", loc);
                        break;
                    } else {
                        step_and_trace(&mut debugger, &mut stack_trace, false);
                    }
                }
            } else if input.starts_with("s:") {
                // Step x times
                match usize::from_str_radix(&input[2..].trim(), 10) {
                    Ok(num) => {
                        for _ in 0..num {
                            step_and_trace(&mut debugger, &mut stack_trace, true);
                        }
                    },
                    Err(e) => println!("Invalid number of steps: {}", e),
                }
            } else if input.starts_with("s") {
                // Step
                step_and_trace(&mut debugger, &mut stack_trace, true);
            } else if input.starts_with("p:") {
                // Print cpu or mem state
                print(&input[2..].trim(), &mut debugger);
            } else if input.starts_with("p") {
                // Print state
                print_all(&mut debugger);
            } else if input.starts_with("t") {
                let trace = stack_trace.iter()
                    .map(|n| format!("${:08X}", n))
                    .collect::<Vec<_>>()
                    .join("\n");
                println!("{}", trace);
            } else if input.starts_with("h") {
                // Help
                help();
            } else if input.starts_with("q") {
                break;
            },
            Err(e) => println!("Input error: {}", e),
        }
    }
}

fn print(s: &str, debugger: &mut PSDebugger) {
    if let Some(reg) = s.strip_prefix("r") {
        if reg == "pc" {
            println!("pc: ${:08X}", debugger.get_state().pc);
        } else if reg == "hi" {
            println!("hi: ${:08X}", debugger.get_state().hi);
        } else if reg == "lo" {
            println!("lo: ${:08X}", debugger.get_state().lo);
        } else {
            match usize::from_str_radix(reg, 10) {
                Ok(num) => println!("r{}: ${:08X}", num, debugger.get_state().regs[num]),
                Err(e) => println!("Invalid p tag: {}", e),
            }
        }
    } else if let Some(bytes) = s.strip_prefix("b") {
        // Memory range
        if let Some(x) = bytes.find('-') {
            match u32::from_str_radix(&bytes[..x], 16) {
                Ok(start) => match u32::from_str_radix(&s[(x+1)..], 16) {
                    Ok(end) => {
                        println!("${:08X} - ${:08X}:", start, end);
                        let mems = (start..end).map(|n| format!("{:02X}", debugger.read_byte(n)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!("{}", mems);
                    },
                    Err(e) => println!("Invalid p tag: {}", e),
                },
                Err(e) => println!("Invalid p tag: {}", e),
            }
        } else {    // Single location
            match u32::from_str_radix(bytes, 16) {
                Ok(num) => println!("${:08X}: ${:02X}", num, debugger.read_byte(num)),
                Err(e) => println!("Invalid p tag: {}", e),
            }
        }
    } else if let Some(words) = s.strip_prefix("w") {
        // Memory range
        if let Some(x) = words.find('-') {
            match u32::from_str_radix(&words[..x], 16) {
                Ok(start) => match u32::from_str_radix(&s[(x+1)..], 16) {
                    Ok(end) => {
                        println!("${:08X} - ${:08X}:", start, end);
                        let mems = (start..end).map(|n| format!("{:08X}", debugger.read_word(n)))
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!("{}", mems);
                    },
                    Err(e) => println!("Invalid p tag: {}", e),
                },
                Err(e) => println!("Invalid p tag: {}", e),
            }
        } else {    // Single location
            match u32::from_str_radix(words, 16) {
                Ok(num) => println!("${:08X}: ${:08X}", num, debugger.read_word(num)),
                Err(e) => println!("Invalid p tag: {}", e),
            }
        }
    } else {
        println!("unrecognised printable")
    }
}

fn print_all(debug_interface: &mut PSDebugger) {
    let state = debug_interface.get_state();
    println!(" 0: {:08X} {:08X} {:08X} {:08X}", state.regs[0], state.regs[1], state.regs[2], state.regs[3]);
    println!(" 4: {:08X} {:08X} {:08X} {:08X}", state.regs[4], state.regs[5], state.regs[6], state.regs[7]);
    println!(" 8: {:08X} {:08X} {:08X} {:08X}", state.regs[8], state.regs[9], state.regs[10], state.regs[11]);
    println!("12: {:08X} {:08X} {:08X} {:08X}", state.regs[12], state.regs[13], state.regs[14], state.regs[15]);
    println!("16: {:08X} {:08X} {:08X} {:08X}", state.regs[16], state.regs[17], state.regs[18], state.regs[19]);
    println!("20: {:08X} {:08X} {:08X} {:08X}", state.regs[20], state.regs[21], state.regs[22], state.regs[23]);
    println!("24: {:08X} {:08X} {:08X} {:08X}", state.regs[24], state.regs[25], state.regs[26], state.regs[27]);
    println!("28: {:08X} {:08X} {:08X} {:08X}", state.regs[28], state.regs[29], state.regs[30], state.regs[31]);
    println!("pc: {:08X} hi: {:08X} lo: {:08X}", state.pc, state.hi, state.lo);
}

fn help() {
    println!("b:x: New breakpoint at memory location x (hex).");
    println!("c:x: Clear breakpoint at memory location x (hex).");
    println!("r: Keep running until a breakpoint is hit.");
    println!("s: Step a single instruction, and see the current instruction pipeline.");
    println!("s:x: Step multiple instructions (base 10).");
    println!("t: Print the stack trace (all the call locations).");
    println!("p: Print the current state of the CPU.");
    println!("p:rx: Print the register x.");
    println!("p:bx: Print the byte found at address x.");
    println!("p:bx-y: Print the memory in the range x -> y.");
    println!("q: Quit execution.");
}

// Step the CPU, and add the PC to the stack trace if it calls.
fn step_and_trace(debugger: &mut PSDebugger, _stack_trace: &mut Vec<u32>, print: bool) {
    let state = debugger.get_state();
    
    if print {
        if let Some(instr) = state.instr {
            println!("${:08X} {}", state.pc, instr);
        } else {
            println!("${:08X} INVALID", state.pc);
        }
    }

    debugger.step();
}
