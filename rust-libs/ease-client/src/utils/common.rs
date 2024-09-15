use ease_client_shared::MusicDuration;

pub fn get_display_duration(duration: &Option<MusicDuration>) -> String {
    if duration.is_none() {
        return "-:-:-".to_string();
    }

    let duration = duration.unwrap();
    let hours = duration.as_secs() / 3600;
    let minutes = duration.as_secs() / 60 % 60;
    let seconds = duration.as_secs() % 60;

    return format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
}
