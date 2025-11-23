use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug)]
#[diesel(table_name = crate::schema::garbage)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Garbage {
    pub id: i32,
    pub garbage_name: String,
}
