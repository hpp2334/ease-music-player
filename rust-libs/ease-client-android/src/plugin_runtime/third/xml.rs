//! `xml_host` bodies — quick-xml → a serde_json tree (the
//! PROPFIND/multistatus decoder; the JS epoch's `fast-xml-parser`
//! npm-dep replacement). Identity-free.

use std::collections::HashMap;

/// Bind the body under the `xml_host` scope (scope-relative key).
pub(crate) fn pkg() -> rut_vm::interp::HostPkg {
    let mut pkg = rut_vm::interp::HostPkg::new("xml_host");

    rut_vm::pkg_fn!(pkg, "to_json", (Vec<u8>,) -> Option<String>, move |_vm: &mut rut_vm::interp::Vm, data: Vec<u8>| {
        Ok(xml_to_json(&data))
    });

    pkg
}

/// The shared body: quick-xml → JSON
/// tree. Element: `{"attrs…": …, "text": "…", "children": {tag: el |
/// [el, …]}}`. Namespace prefixes stay verbatim (the plugins match
/// `D:multistatus` / `d:response` case-insensitively downstream).
/// `None` = unparseable XML or invalid UTF-8.
pub(crate) fn xml_to_json(bytes: &[u8]) -> Option<String> {
    use quick_xml::events::Event;

    let text = std::str::from_utf8(bytes).ok()?;
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);

    #[derive(Default)]
    struct Node {
        attrs: serde_json::Map<String, serde_json::Value>,
        text: String,
        children: HashMap<String, Vec<serde_json::Value>>,
    }

    fn finish(n: Node) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        for (k, v) in n.attrs {
            obj.insert(k, v);
        }
        if !n.text.is_empty() {
            obj.insert("text".into(), serde_json::Value::String(n.text));
        }
        if !n.children.is_empty() {
            obj.insert(
                "children".into(),
                serde_json::Value::Object(
                    n.children
                        .into_iter()
                        .map(|(k, mut v)| {
                            if v.len() == 1 {
                                (k, v.pop().unwrap_or(serde_json::Value::Null))
                            } else {
                                (k, serde_json::Value::Array(v))
                            }
                        })
                        .collect(),
                ),
            );
        }
        serde_json::Value::Object(obj)
    }

    let mut stack: Vec<(String, Node)> = vec![(String::new(), Node::default())];
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut node = Node::default();
                for a in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    node.attrs.insert(
                        k,
                        serde_json::Value::String(
                            String::from_utf8_lossy(a.unescape_value().unwrap_or_default().as_bytes())
                                .to_string(),
                        ),
                    );
                }
                stack.push((name, node));
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut node = Node::default();
                for a in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    node.attrs.insert(
                        k,
                        serde_json::Value::String(
                            String::from_utf8_lossy(a.unescape_value().unwrap_or_default().as_bytes())
                                .to_string(),
                        ),
                    );
                }
                if let Some((_, top)) = stack.last_mut() {
                    top.children.entry(name).or_default().push(finish(node));
                }
            }
            Ok(Event::End(_)) => {
                if stack.len() <= 1 {
                    break;
                }
                let (name, node) = stack.pop().expect("stack non-empty");
                if let Some((_, top)) = stack.last_mut() {
                    top.children.entry(name).or_default().push(finish(node));
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(txt) = t.unescape() {
                    if let Some((_, top)) = stack.last_mut() {
                        top.text.push_str(&txt);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            Ok(_) => {}
        }
        buf.clear();
    }

    if stack.len() != 1 {
        return None; // unbalanced document
    }
    let (_, root) = stack.pop().expect("stack non-empty");
    Some(finish(root).to_string())
}
