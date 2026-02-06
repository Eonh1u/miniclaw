use anyhow::Result;
use rustyline::DefaultEditor;
use crate::agent::Agent;

pub async fn run_chat_loop(mut agent: Agent) -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    println!("Type your message. Type /quit to exit.");
    loop {
        match rl.readline("You > ") {
            Ok(line) => {
                let input = line.trim().to_string();
                if input.is_empty() { continue; }
                if input == "/quit" { break; }
                if input == "/clear" {
                    agent.clear_history();
                    println!("[Cleared]");
                    continue;
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
