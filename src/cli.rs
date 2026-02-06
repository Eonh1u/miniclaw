use anyhow::Result;
use rustyline::DefaultEditor;
use crate::agent::Agent;

pub async fn run_chat_loop(mut agent: Agent) -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    println!("Type your message. Type quit or exit to leave, /clear to reset.");
    loop {
        match rl.readline("You > ") {
            Ok(line) => {
                let input = line.trim().to_string();
                if input.is_empty() { continue; }
                match input.to_lowercase().as_str() {
                    "quit" | "exit" | "/quit" | "/exit" => {
                        println!("Goodbye!");
                        break;
                    }
                    "/clear" | "clear" => {
                        agent.clear_history();
                        println!("[Cleared]");
                        continue;
                    }
                    _ => {}
                }
                let _ = rl.add_history_entry(&input);
                match agent.process_message(&input).await {
                    Ok(r) => println!("\nAssistant > {}\n", r),
                    Err(e) => println!("\n[Error: {}]\n", e),
                }
            }
            Err(_) => break,
        }
    }
    Ok(())
}
