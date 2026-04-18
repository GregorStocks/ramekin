use axum::Json;
use serde_json::{json, Value};

pub async fn apple_app_site_association() -> Json<Value> {
    Json(json!({
        "applinks": {
            "details": [{
                "appIDs": ["32ANM8P9HJ.com.ramekin.app"],
                "components": [{"/": "/recipes/*"}]
            }]
        }
    }))
}
