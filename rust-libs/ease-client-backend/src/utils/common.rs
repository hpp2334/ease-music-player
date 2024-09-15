pub fn decode_component_or_origin(s: String) -> String {
    let res = urlencoding::decode(&s);
    if let Ok(res) = res {
        res.to_string()
    } else {
        s
    }
}
