//! A minimal valid DocIR document for the tests of this crate.

/// One invoice with one line of text and one accepted `total` amount.
pub fn document(id: &str, text: &str, total: &str, total_ms: f64) -> String {
    format!(
        r#"{{"schema":"wanjiru.docir/0","id":"{id}","class":"invoice","source":{{"sha256":"{zero}","media_type":"image/png"}},"pages":[{{"index":0,"size":{{"w":100,"h":200}},"chain":{{"from":"source","steps":[]}},"lines":[{{"id":"l0","text":"{text}","polygon":[[0,0],[100,0],[100,8],[0,8]],"confidence":{{"value":0.9,"calibrated":false}},"words":[]}}]}}],"fields":[{{"name":"total","kind":"amount","value":"{total}","confidence":{{"value":0.9,"calibrated":false}},"decision":"accept","evidence":[],"lines":["l0"]}}],"run":{{"engine":"test 0","config_sha256":"{one}","timing_ms":{{"total":{total_ms}}}}}}}"#,
        zero = "0".repeat(64),
        one = "1".repeat(64),
    )
}
