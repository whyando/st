use serde::{Deserialize, Deserializer, Serialize};

use super::SystemSymbol;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Faction {
    pub symbol: String,
    pub name: String,
    pub description: String,
    #[serde(deserialize_with = "empty_string_is_none")]
    pub headquarters: Option<SystemSymbol>,
    pub traits: Vec<Trait>,
    pub is_recruiting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trait {
    pub symbol: String,
    pub name: String,
    pub description: String,
}

fn empty_string_is_none<'de, D>(deserializer: D) -> Result<Option<SystemSymbol>, D::Error>
where
    D: Deserializer<'de>,
{
    // deserialize as Option<String>
    let opt_string: Option<String> = Option::deserialize(deserializer)?;
    // convert to Option<SystemSymbol>, None if empty string
    let system = match opt_string {
        Some(s) => {
            if s.is_empty() {
                None
            } else {
                Some(SystemSymbol(s))
            }
        }
        None => None,
    };
    Ok(system)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_empty_string_is_none() {
        let faction_json = r#"{"symbol":"ECHO","name":"Echo Technological Conclave","description":"Echo Technological Conclave is an innovative and forward-thinking faction that thrives on technological advancement and scientific discovery. They have a deep commitment to progress and a drive to push the boundaries of what is possible, making them a force to be reckoned with.","headquarters":"","traits":[{"symbol":"INNOVATIVE","name":"Innovative","description":"Willing to try new and untested ideas. Sometimes able to come up with creative and original solutions to problems, and may be able to think outside the box. Sometimes at the forefront of technological or social change, and may be willing to take risks in order to advance the boundaries of human knowledge and understanding."},{"symbol":"VISIONARY","name":"Visionary","description":"Possessing a clear and compelling vision for the future. Sometimes able to see beyond the present and anticipate the needs and challenges of tomorrow. Sometimes able to inspire and guide others towards a better and brighter future, and may be willing to take bold and decisive action to make their vision a reality."},{"symbol":"RESEARCH_FOCUSED","name":"Research-Focused","description":"Dedicated to advancing knowledge and understanding through research and experimentation. Often have a strong focus on scientific and technological development, and may be willing to take risks and explore new ideas in order to make progress."},{"symbol":"TECHNOLOGICALLY_ADVANCED","name":"Technologically Advanced","description":"Possessing advanced technology and knowledge, often far beyond the level of other factions. Often have access to powerful weapons, ships, and other technology that gives them a significant advantage in battles and other conflicts."}],"isRecruiting":false}"#;
        let faction: Faction = serde_json::from_str(faction_json).unwrap();
        assert_eq!(faction.headquarters, None);

        let faction_json = serde_json::to_string(&faction).unwrap();
        let _faction: Faction = serde_json::from_str(&faction_json).unwrap();
    }
}
