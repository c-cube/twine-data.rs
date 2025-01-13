use serde_json::Value as JV;
use twine_data::value::Value as TV;

fn same_value(j: &JV, t: &TV) -> bool {
    match (j, t) {
        (JV::Null, TV::Null) => true,
        (JV::Null, _) => false,
        (JV::Bool(b1), TV::Bool(b2)) => b1 == b2,
        (JV::Bool(_), _) => false,
        (JV::Number(n), TV::Int64(i)) if n.is_i64() => *i == n.as_i64().unwrap(),
        (JV::Number(n), TV::Float(n2)) if n.is_f64() => *n2 == n.as_f64().unwrap(),
        (JV::Number(_), _) => false,
        (JV::String(s1), TV::String(s2)) => s1 == s2,
        (JV::String(_), _) => false,
        (JV::Array(vec), TV::Array(vec2)) => {
            (vec.len() == vec2.len()) && vec.iter().zip(vec2.iter()).all(|(j, t)| same_value(j, t))
        }
        (JV::Array(_), _) => false,
        (JV::Object(map), TV::Map(map2)) => {
            (map.len() == map2.len())
                && (map.iter().zip(map2.iter())).all(|((k1, v1), (k2, v2))| {
                    same_value(&JV::String(k1.clone()), k2) && same_value(v1, v2)
                })
        }
        (JV::Object(_), _) => false,
    }
}

#[test]
pub fn test_same() {
    let json_data = std::fs::read("tests/twitter.json").unwrap();
    let json: serde_json::Value = serde_json::from_slice(&json_data).unwrap();

    let twine_data = std::fs::read("tests/twitter.twine").unwrap();
    let tv = twine_data::value::read_value_from_entrypoint(
        &twine_data::Decoder::new(&twine_data).unwrap(),
    )
    .expect("cannot read twine");

    println!(
        "comparing {}B of json with {}B of twine…",
        json_data.len(),
        twine_data.len()
    );
    let same = same_value(&json, &tv);
    assert!(same);
}
