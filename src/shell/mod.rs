use alloc::string::String;
use alloc::vec::Vec;
use alloc::string::ToString;
use alloc::format;
use core::cmp::min;
use crate::println;
use crate::fs;
use crate::vga_buffer;

#[derive(Debug)]
pub struct Command {
    name: String,
    args: Vec<String>,
}

impl Command {
    pub fn new(input: &str) -> Option<Self> {
        let mut parts = input.split_whitespace();
        let name = parts.next()?.to_string();
        let args: Vec<String> = parts.map(|s| s.to_string()).collect();
        
        Some(Command { name, args })
    }
}

pub struct Shell {
    current_dir: String,
    command_history: Vec<String>,
    history_position: Option<usize>,
    tab_completions: Vec<String>,
    tab_index: usize,
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            current_dir: "/".to_string(),
            command_history: Vec::new(),
            history_position: None,
            tab_completions: Vec::new(),
            tab_index: 0,
        }
    }

    // Add tab completion function
    pub fn tab_complete(&mut self, input: &str) -> Option<String> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        
        // If this is the first tab press, generate completions
        if self.tab_completions.is_empty() {
            let (prefix, path_to_complete) = if parts.is_empty() {
                ("", "")
            } else if parts.len() == 1 {
                // Completing a command
                (parts[0], "")
            } else {
                // Completing a path
                (parts[parts.len() - 1], parts[parts.len() - 1])
            };

            self.generate_completions(prefix, path_to_complete);
            self.tab_index = 0;
        } else {
            // Cycle through existing completions
            self.tab_index = (self.tab_index + 1) % self.tab_completions.len();
        }

        if let Some(completion) = self.tab_completions.get(self.tab_index) {
            // If completing a path argument, replace only the last part
            if parts.len() > 1 {
                let mut new_parts = parts[..parts.len()-1].to_vec();
                new_parts.push(completion);
                Some(new_parts.join(" "))
            } else {
                Some(completion.clone())
            }
        } else {
            None
        }
    }

    // Reset tab completion state
    pub fn reset_tab_completion(&mut self) {
        self.tab_completions.clear();
        self.tab_index = 0;
    }

    fn generate_completions(&mut self, prefix: &str, path_to_complete: &str) {
        self.tab_completions.clear();

        if path_to_complete.is_empty() {
            // Complete commands
            for cmd in ["ls", "cd", "pwd", "help", "clear", "cat", "mkdir", "touch", "rm", "echo", "cp", "mv"] {
                if cmd.starts_with(prefix) {
                    self.tab_completions.push(cmd.to_string());
                }
            }
        } else {
            // Complete paths
            let (dir_path, file_prefix) = self.split_path(path_to_complete);
            let search_dir = if dir_path.is_empty() {
                self.current_dir.clone()
            } else {
                self.resolve_path(&dir_path)
            };

            let fs = fs::ROOT_FS.read();
            if let Ok(entries) = fs.read_dir(&search_dir) {
                for entry in entries {
                    if entry.starts_with(file_prefix) {
                        let full_path = if dir_path.is_empty() {
                            entry
                        } else {
                            format!("{}/{}", dir_path, entry)
                        };
                        self.tab_completions.push(full_path);
                    }
                }
            }
        }

        self.tab_completions.sort();
    }

    fn split_path(&self, path: &str) -> (String, &str) {
        if let Some(last_slash) = path.rfind('/') {
            (path[..last_slash].to_string(), &path[last_slash + 1..])
        } else {
            (String::new(), path)
        }
    }

    // Add new file operations
    fn cmd_cp(&self, args: &[String]) {
        if args.len() != 2 {
            println!("cp: missing file operand");
            println!("Usage: cp <source> <destination>");
            return;
        }

        let src_path = self.resolve_path(&args[0]);
        let dst_path = self.resolve_path(&args[1]);
        let fs = fs::ROOT_FS.read();

        // Read source file
        match fs.read_file(&src_path) {
            Ok(contents) => {
                // Write to destination
                if let Err(e) = fs.create_file(&dst_path, contents) {
                    println!("cp: error writing to {}: {}", args[1], e);
                }
            }
            Err(e) => println!("cp: error reading {}: {}", args[0], e),
        }
    }

    fn cmd_mv(&self, args: &[String]) {
        if args.len() != 2 {
            println!("mv: missing file operand");
            println!("Usage: mv <source> <destination>");
            return;
        }

        let src_path = self.resolve_path(&args[0]);
        let dst_path = self.resolve_path(&args[1]);
        let fs = fs::ROOT_FS.read();

        // First try to read the source file
        match fs.read_file(&src_path) {
            Ok(contents) => {
                // Create the destination file
                if let Err(e) = fs.create_file(&dst_path, contents) {
                    println!("mv: error writing to {}: {}", args[1], e);
                    return;
                }
                // Remove the source file
                if let Err(e) = fs.remove_file(&src_path) {
                    println!("mv: error removing source file {}: {}", args[0], e);
                }
            }
            Err(e) => println!("mv: error reading {}: {}", args[0], e),
        }
    }

    // Update execute to include new commands
    pub fn execute(&mut self, input: &str) {
        if input.trim().is_empty() {
            return;
        }

        // Add command to history
        self.command_history.push(input.to_string());
        self.history_position = None;

        let command = match Command::new(input) {
            Some(cmd) => cmd,
            None => return,
        };

        match command.name.as_str() {
            "ls" => self.cmd_ls(&command.args),
            "cd" => self.cmd_cd(&command.args),
            "pwd" => self.cmd_pwd(),
            "help" => self.cmd_help(),
            "clear" => self.cmd_clear(),
            "cat" => self.cmd_cat(&command.args),
            "mkdir" => self.cmd_mkdir(&command.args),
            "touch" => self.cmd_touch(&command.args),
            "rm" => self.cmd_rm(&command.args),
            "echo" => self.cmd_echo(&command.args),
            "cp" => self.cmd_cp(&command.args),
            "mv" => self.cmd_mv(&command.args),
            _ => println!("Unknown command: {}", command.name),
        }
    }

    // Update help to include new commands
    fn cmd_help(&self) {
        println!("Available commands:");
        println!("  ls [path]     - List directory contents");
        println!("  cd [path]     - Change current directory");
        println!("  pwd           - Print current directory");
        println!("  cat <file>    - Display file contents");
        println!("  mkdir <dir>   - Create a directory");
        println!("  touch <file>  - Create an empty file");
        println!("  rm <file>     - Remove a file");
        println!("  echo [text]   - Display a line of text");
        println!("  cp <src> <dst> - Copy a file");
        println!("  mv <src> <dst> - Move a file");
        println!("  clear         - Clear the screen");
        println!("  help          - Show this help message");
        println!("\nUse Tab for command/path completion");
    }

    // Previous command navigation
    pub fn previous_command(&mut self) -> Option<&str> {
        if self.command_history.is_empty() {
            return None;
        }

        match self.history_position {
            None => {
                self.history_position = Some(self.command_history.len() - 1);
            }
            Some(pos) if pos > 0 => {
                self.history_position = Some(pos - 1);
            }
            _ => return None,
        }

        self.command_history.get(self.history_position?)
            .map(|s| s.as_str())
    }

    // Next command navigation
    pub fn next_command(&mut self) -> Option<&str> {
        match self.history_position {
            Some(pos) if pos < self.command_history.len() - 1 => {
                self.history_position = Some(pos + 1);
                self.command_history.get(pos + 1).map(|s| s.as_str())
            }
            _ => {
                self.history_position = None;
                Some("")
            }
        }
    }

    fn resolve_path(&self, path: &str) -> String {
        let fs = fs::ROOT_FS.read();
        fs.canonicalize_path(&self.current_dir, path)
            .unwrap_or_else(|_| path.to_string())
    }

    // Existing commands...
    fn cmd_ls(&self, args: &[String]) {
        let path = if args.is_empty() {
            &self.current_dir
        } else {
            &args[0]
        };

        let fs = fs::ROOT_FS.read();
        match fs.read_dir(path) {
            Ok(entries) => {
                for entry in entries {
                    println!("{}", entry);
                }
            }
            Err(e) => println!("ls: {}: {}", path, e),
        }
    }

    fn cmd_cd(&mut self, args: &[String]) {
        let path = args.get(0).map(|s| s.as_str()).unwrap_or("/");
        let fs = fs::ROOT_FS.read();
        
        match fs.canonicalize_path(&self.current_dir, path) {
            Ok(new_path) => {
                if fs.is_dir(&new_path) {
                    self.current_dir = new_path;
                } else {
                    println!("cd: {}: Not a directory", path);
                }
            }
            Err(e) => println!("cd: {}: {}", path, e),
        }
    }

    // New commands...
    fn cmd_cat(&self, args: &[String]) {
        if args.is_empty() {
            println!("cat: missing file operand");
            return;
        }

        let fs = fs::ROOT_FS.read();
        for path in args {
            let full_path = self.resolve_path(path);
            match fs.read_file(&full_path) {
                Ok(contents) => {
                    // Convert bytes to string and print
                    for byte in contents {
                        print!("{}", byte as char);
                    }
                    println!();
                }
                Err(e) => println!("cat: {}: {}", path, e),
            }
        }
    }

    fn cmd_mkdir(&self, args: &[String]) {
        if args.is_empty() {
            println!("mkdir: missing operand");
            return;
        }

        let fs = fs::ROOT_FS.read();
        for dir in args {
            let full_path = self.resolve_path(dir);
            if let Err(e) = fs.create_dir(&full_path) {
                println!("mkdir: {}: {}", dir, e);
            }
        }
    }

    fn cmd_touch(&self, args: &[String]) {
        if args.is_empty() {
            println!("touch: missing file operand");
            return;
        }

        let fs = fs::ROOT_FS.read();
        for file in args {
            let full_path = self.resolve_path(file);
            if let Err(e) = fs.create_file(&full_path, Vec::new()) {
                println!("touch: {}: {}", file, e);
            }
        }
    }

    fn cmd_rm(&self, args: &[String]) {
        if args.is_empty() {
            println!("rm: missing operand");
            return;
        }

        let fs = fs::ROOT_FS.read();
        for path in args {
            let full_path = self.resolve_path(path);
            if let Err(e) = fs.remove_file(&full_path) {
                println!("rm: {}: {}", path, e);
            }
        }
    }

    fn cmd_echo(&self, args: &[String]) {
        let text = args.join(" ");
        println!("{}", text);
    }

    fn cmd_pwd(&self) {
        println!("{}", self.current_dir);
    }

    fn cmd_clear(&self) {
        vga_buffer::WRITER.lock().clear_screen();
    }
}

pub fn init() -> Shell {
    Shell::new()
} 