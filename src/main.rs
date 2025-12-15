use clap::Parser;
use crossterm::{
    ExecutableCommand, QueueableCommand, cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Color, Print, SetForegroundColor, Stylize},
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};
use std::io::{self, Write};
use std::process::Command;
use unicode_width::UnicodeWidthChar;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Optional query to start with
    query: Option<String>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    // Store the initial query
    let mut current_query = args.query;

    loop {
        // 1. Get Input
        // If current_query is Some (from args or previous edit), use it.
        // Otherwise, prompt user.
        let query = if let Some(q) = current_query.take() {
            q
        } else {
            get_multiline_input()?
        };

        if query.is_empty() {
            // If user enters empty string at prompt, maybe exit or loop?
            // Let's assume exit if it was interactive prompt
            break;
        }

        // 2. Mock AI Response
        let explanation = "To check disk space usage, you can use the 'df' command. It displays the amount of disk space used and available on file systems.";
        // Fixed command as requested
        let command_str = "df".to_string();

        // 3. Display Result
        println!();
        println!("{}", explanation.blue());
        println!("{}", command_str.clone().green().bold());
        println!();

        // 4. Interactive Menu
        enable_raw_mode()?;
        let mut stdout = io::stdout();

        let options = ["Execute", "Edit", "Cancel"];
        let mut selection = 0;
        let mut should_exit_loop = false;

        loop {
            // Render menu
            for (i, option) in options.iter().enumerate() {
                stdout.queue(cursor::MoveToColumn(0))?;
                if i == selection {
                    stdout.queue(SetForegroundColor(Color::Green))?;
                    stdout.queue(Print("> "))?;
                    stdout.queue(Print(option))?;
                    stdout.queue(SetForegroundColor(Color::Reset))?;
                } else {
                    stdout.queue(Print("  "))?;
                    stdout.queue(Print(option))?;
                }
                stdout.queue(Print("\r\n"))?;
            }
            stdout.flush()?;

            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Up => {
                        if selection > 0 {
                            selection -= 1;
                        } else {
                            selection = options.len() - 1;
                        }
                    }
                    KeyCode::Down => {
                        if selection < options.len() - 1 {
                            selection += 1;
                        } else {
                            selection = 0;
                        }
                    }
                    KeyCode::Enter => {
                        break;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        selection = 2; // Cancel
                        break;
                    }
                    _ => {}
                }
            }

            // Clear the menu lines so we can redraw
            stdout.queue(cursor::MoveUp(options.len() as u16))?;
            stdout.queue(Clear(ClearType::FromCursorDown))?;
        }

        // Cleanup raw mode
        disable_raw_mode()?;

        // Clear menu area
        stdout.execute(cursor::MoveUp(options.len() as u16))?;
        stdout.execute(Clear(ClearType::FromCursorDown))?;

        match selection {
            0 => {
                // Execute
                println!("Executing: {}", command_str);
                execute_command(&command_str)?;
                should_exit_loop = true;
            }
            1 => {
                // Edit
                // Instead of editing the command directly, we loop back to prompt for new query.
                // We do NOT set current_query, so the next loop iteration will trigger the prompt.
                continue;
            }
            2 => {
                // Cancel
                println!("Cancelled.");
                should_exit_loop = true;
            }
            _ => {}
        }

        if should_exit_loop {
            break;
        }
    }

    cleanup();
    Ok(())
}

fn execute_command(cmd: &str) -> io::Result<()> {
    let output = Command::new("sh").arg("-c").arg(cmd).output();

    match output {
        Ok(out) => {
            io::stdout().write_all(&out.stdout)?;
            io::stderr().write_all(&out.stderr)?;
        }
        Err(e) => {
            eprintln!("Failed to execute command: {}", e);
        }
    }
    Ok(())
}

fn get_multiline_input() -> io::Result<String> {
    println!("Type your query (Press Enter to submit, Alt+Enter to newline, Ctrl+C to cancel):");
    print!("? ");
    io::stdout().flush()?;

    enable_raw_mode()?;
    let start_pos = cursor::position()?;
    let mut state = InputState::new(start_pos);
    let mut stdout = io::stdout();

    loop {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if state.buffer.is_empty() {
                        disable_raw_mode()?;
                        cleanup();
                        std::process::exit(0);
                    } else {
                        state.delete();
                        state.render(&mut stdout)?;
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    disable_raw_mode()?;
                    cleanup();
                    std::process::exit(0);
                }
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl+J -> Newline
                    state.insert('\n');
                    state.render(&mut stdout)?;
                }
                KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.move_home();
                    state.render(&mut stdout)?;
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.move_end();
                    state.render(&mut stdout)?;
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.kill_line();
                    state.render(&mut stdout)?;
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.move_right();
                    state.render(&mut stdout)?;
                }
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.move_left();
                    state.render(&mut stdout)?;
                }
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.move_down();
                    state.render(&mut stdout)?;
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.move_up();
                    state.render(&mut stdout)?;
                }
                KeyCode::Char(c) => {
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT)
                    {
                        state.insert(c);
                        state.render(&mut stdout)?;
                    }
                }
                KeyCode::Enter => {
                    if key.modifiers.contains(KeyModifiers::ALT)
                        || key.modifiers.contains(KeyModifiers::SHIFT)
                    {
                        state.insert('\n');
                        state.render(&mut stdout)?;
                    } else {
                        break;
                    }
                }
                KeyCode::Backspace => {
                    state.backspace();
                    state.render(&mut stdout)?;
                }
                KeyCode::Delete => {
                    state.delete();
                    state.render(&mut stdout)?;
                }
                KeyCode::Left => {
                    state.move_left();
                    state.render(&mut stdout)?;
                }
                KeyCode::Right => {
                    state.move_right();
                    state.render(&mut stdout)?;
                }
                KeyCode::Up => {
                    state.move_up();
                    state.render(&mut stdout)?;
                }
                KeyCode::Down => {
                    state.move_down();
                    state.render(&mut stdout)?;
                }
                _ => {}
            }
        }
    }

    // Move to end before exiting
    let (_, rows) = state.get_visual_pos(state.buffer.len());
    let (_curr_col, curr_row) = state.get_visual_pos(state.cursor);
    if rows > curr_row {
        stdout.execute(cursor::MoveDown(rows - curr_row))?;
    }
    stdout.execute(cursor::MoveToColumn(0))?;

    disable_raw_mode()?;
    println!(); // Ensure final newline
    Ok(state
        .buffer
        .into_iter()
        .collect::<String>()
        .trim()
        .to_string())
}

fn cleanup() {
    println!("Bye!");
}

struct InputState {
    buffer: Vec<char>,
    cursor: usize,
    start_pos: (u16, u16),
}

impl InputState {
    fn new(start_pos: (u16, u16)) -> Self {
        Self {
            buffer: Vec::new(),
            cursor: 0,
            start_pos,
        }
    }

    fn insert(&mut self, c: char) {
        if self.cursor > self.buffer.len() {
            self.cursor = self.buffer.len();
        }
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.buffer.remove(self.cursor);
        }
    }

    fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
        }
    }

    fn current_line_range(&self) -> (usize, usize) {
        let start = self.buffer[..self.cursor]
            .iter()
            .rposition(|&c| c == '\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = self.buffer[self.cursor..]
            .iter()
            .position(|&c| c == '\n')
            .map(|i| self.cursor + i)
            .unwrap_or(self.buffer.len());
        (start, end)
    }

    fn move_home(&mut self) {
        let (start, _) = self.current_line_range();
        self.cursor = start;
    }

    fn move_end(&mut self) {
        let (_, end) = self.current_line_range();
        self.cursor = end;
    }

    fn kill_line(&mut self) {
        let (_, end) = self.current_line_range();
        if end > self.cursor {
            self.buffer.drain(self.cursor..end);
        } else if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    fn move_up(&mut self) {
        let (start, _) = self.current_line_range();
        if start == 0 {
            return;
        }

        // Calculate visual offset of current cursor relative to line start
        let mut col_offset = 0;
        for &c in &self.buffer[start..self.cursor] {
            col_offset += UnicodeWidthChar::width(c).unwrap_or(0);
        }

        let prev_line_end = start - 1;
        let prev_line_start = self.buffer[..prev_line_end]
            .iter()
            .rposition(|&c| c == '\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        
        // Find position in previous line that matches visual offset
        let mut current_offset = 0;
        let mut target_cursor = prev_line_start;
        
        for (i, &c) in self.buffer[prev_line_start..prev_line_end].iter().enumerate() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if current_offset + w > col_offset {
                 // Closest match
                 break;
            }
            current_offset += w;
            target_cursor = prev_line_start + i + 1;
        }
        
        self.cursor = target_cursor;
    }

    fn move_down(&mut self) {
        let (_, end) = self.current_line_range();
        if end == self.buffer.len() {
            return;
        }

        let (start, _) = self.current_line_range();
        
        // Calculate visual offset of current cursor relative to line start
        let mut col_offset = 0;
        for &c in &self.buffer[start..self.cursor] {
            col_offset += UnicodeWidthChar::width(c).unwrap_or(0);
        }

        let next_line_start = end + 1;
        let next_line_end = self.buffer[next_line_start..]
            .iter()
            .position(|&c| c == '\n')
            .map(|i| next_line_start + i)
            .unwrap_or(self.buffer.len());
            
        // Find position in next line that matches visual offset
        let mut current_offset = 0;
        let mut target_cursor = next_line_start;
        
        for (i, &c) in self.buffer[next_line_start..next_line_end].iter().enumerate() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
             if current_offset + w > col_offset {
                 break;
            }
            current_offset += w;
            target_cursor = next_line_start + i + 1;
        }

        self.cursor = target_cursor;
    }

    fn get_visual_pos(&self, index: usize) -> (u16, u16) {
        let mut col = 0;
        let mut row = 0;
        for (i, &c) in self.buffer.iter().enumerate() {
            if i == index {
                break;
            }
            if c == '\n' {
                row += 1;
                col = 0;
            } else {
                col += UnicodeWidthChar::width(c).unwrap_or(0) as u16;
            }
        }
        (col, row)
    }

    fn render(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
        // 1. Move to start
        stdout.queue(cursor::MoveTo(self.start_pos.0, self.start_pos.1))?;

        // 2. Clear and Print
        stdout.queue(Clear(ClearType::FromCursorDown))?;
        for c in &self.buffer {
            if *c == '\n' {
                stdout.queue(Print("\r\n"))?;
            } else {
                stdout.queue(Print(c))?;
            }
        }
        stdout.flush()?;

        // 3. Check for scrolling
        let end_pos = cursor::position()?;
        let (_, visual_rows) = self.get_visual_pos(self.buffer.len());
        let expected_row = self.start_pos.1 + visual_rows;

        if end_pos.1 < expected_row {
            let diff = expected_row - end_pos.1;
            if self.start_pos.1 >= diff {
                self.start_pos.1 -= diff;
            } else {
                self.start_pos.1 = 0;
            }
        }

        // 4. Move cursor to correct position
        let (cursor_col, cursor_row) = self.get_visual_pos(self.cursor);
        let target_row = self.start_pos.1 + cursor_row;
        let target_col = if cursor_row == 0 {
            self.start_pos.0 + cursor_col
        } else {
            cursor_col
        };

        stdout.execute(cursor::MoveTo(target_col, target_row))?;

        Ok(())
    }
}
