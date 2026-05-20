use crate::character::CharacterCard;

/// What kind of command was entered
#[derive(Debug, Clone)]
pub enum Command {
    /// No command — normal message
    None,
    /// `/exit` or `/quit`
    Quit,
    /// `/help`
    Help,
    /// `/clear` — clear current character's history
    Clear,
    /// `/switch <name>` — switch to another character
    Switch(String),
    /// `/save` — save current session
    Save,
    /// `/load <id>` — load a saved session
    Load(String),
    /// `/cc <name>` — create character card
    CreateChar(String),
    /// `/cw <name>` — create world entry
    CreateWorld(String),
    /// `/self <text>` — set user persona
    SetSelf(String),
    /// `/state <query>` — manage character state
    ManageState(String),
    /// `/export` — export to .md files
    Export,
    /// `/world <name>` — switch world
    World(String),
    /// `/link <char> <world>` — link character to world
    Link(String, String),
    /// `/location <action> <args>` — manage locations
    Location(String),
    /// `?<name>` — show character info
    Info(String),
    /// `?list` — list all characters
    List,
}

/// Parse user input for commands
pub fn parse(input: &str) -> (Command, String) {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return (Command::None, String::new());
    }

    // / commands
    if let Some(rest) = trimmed.strip_prefix('/') {
        let rest = rest.trim();
        let (cmd, _args) = rest.split_once(' ').unwrap_or((rest, ""));
        return match cmd {
            "exit" | "quit" | "q" => (Command::Quit, String::new()),
            "help" | "h" => (Command::Help, String::new()),
            "clear" | "cls" => (Command::Clear, String::new()),
            "switch" | "sw" => (Command::Switch(_args.trim().to_string()), String::new()),
            "save" | "s" => (Command::Save, String::new()),
            "load" | "l" => (Command::Load(_args.trim().to_string()), String::new()),
            "cc" => (Command::CreateChar(_args.trim().to_string()), String::new()),
            "cw" => (Command::CreateWorld(_args.trim().to_string()), String::new()),
            "self" => (Command::SetSelf(_args.trim().to_string()), String::new()),
            "state" => (Command::ManageState(_args.trim().to_string()), String::new()),
            "export" | "exp" => (Command::Export, String::new()),
            "world" | "w" => (Command::World(_args.trim().to_string()), String::new()),
            "link" => {
                let (char_name, world_name) = _args.split_once(' ').unwrap_or((_args, ""));
                (Command::Link(char_name.trim().to_string(), world_name.trim().to_string()), String::new())
            },
            "location" | "loc" => (Command::Location(_args.trim().to_string()), String::new()),
            _ => (Command::None, input.to_string()),
        };
    }

    // ? commands
    if let Some(rest) = trimmed.strip_prefix('?') {
        let rest = rest.trim();
        if rest.is_empty() {
            return (Command::None, input.to_string());
        }
        match rest {
            "list" | "ls" => (Command::List, String::new()),
            "help" | "h" => return (Command::Help, String::new()),
            _ => (Command::Info(rest.to_string()), String::new()),
        }
    } else {
        (Command::None, input.to_string())
    }
}

/// Check if input contains @mentions and inject character info
pub fn expand_mentions(
    input: &str,
    lookup: impl Fn(&str) -> Option<CharacterCard>,
) -> String {
    let mut result = input.to_string();
    let mut start = 0;

    while let Some(at_pos) = result[start..].find('@') {
        let abs_pos = start + at_pos;
        // Extract the name after @
        let name_end = result[abs_pos + 1..]
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .map(|p| abs_pos + 1 + p)
            .unwrap_or(result.len());
        let name = &result[abs_pos + 1..name_end];

        if let Some(card) = lookup(name) {
            let inject = format!(
                "\n[系统: 用户@了角色\"{}\"（性格: {}, 说话风格: {}）]",
                card.meta.name, card.meta.personality, card.meta.speech_style
            );
            result.insert_str(name_end, &inject);
            start = name_end + inject.len();
        } else {
            start = name_end;
        }
    }

    result
}
