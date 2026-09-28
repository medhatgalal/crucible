//! Append one factory sentence to the program record.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crucible_contract::{speech_line, Clock, SPEECH_HEADER};

use crate::message;
use crate::GuidedError;

pub fn speech(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let (role, sentence, text) = match args {
        [role, sentence, text] => (*role, *sentence, *text),
        _ => return Err(message("usage: crucible speech ROLE SENTENCE TEXT")),
    };
    let line = speech_line(clock.now_unix(), role, sentence, text).map_err(message)?;
    let path = root.join("SPEECH.tsv");
    if !path.is_file() {
        fs::write(&path, SPEECH_HEADER)?;
    }
    let mut file = OpenOptions::new().append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    Ok(format!("{}\n", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crucible_contract::FixedClock;

    #[test]
    fn appends_one_record_and_a_refusal_does_not_change_it() {
        let dir = std::env::temp_dir().join(format!(
            "crucible-speech-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let clock = FixedClock::new(42);
        speech(&dir, &["manager", "source", "fix the greeting"], &clock).unwrap();
        let before = fs::read(&dir.join("SPEECH.tsv")).unwrap();
        assert_eq!(
            std::str::from_utf8(&before).unwrap(),
            "epoch\trole\tsentence\ttext\n42\tmanager\tsource\tfix the greeting\n"
        );
        let err = speech(&dir, &["manager", "landed", "no"], &clock).unwrap_err();
        assert!(err.to_string().contains("speech sentence refused"), "{err}");
        assert_eq!(fs::read(&dir.join("SPEECH.tsv")).unwrap(), before);
        let _ = fs::remove_dir_all(&dir);
    }
}
