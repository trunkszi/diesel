use std::error::Error;

use diesel::result::Error::NotFound;

use super::data_structures::*;
use super::table_data::*;
use database::InferConnection;

static RESERVED_NAMES: &[&str] = &[
    "abstract", "alignof", "as", "become", "box", "break", "const", "continue", "crate", "do",
    "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in", "let", "loop",
    "macro", "match", "mod", "move", "mut", "offsetof", "override", "priv", "proc", "pub", "pure",
    "ref", "return", "Self", "self", "sizeof", "static", "struct", "super", "trait", "true",
    "type", "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

pub fn load_table_names(
    database_url: &str,
    schema_name: Option<&str>,
) -> Result<Vec<TableName>, Box<Error>> {
    let connection = try!(InferConnection::establish(database_url));

    match connection {
        #[cfg(feature = "sqlite")]
        InferConnection::Sqlite(c) => super::sqlite::load_table_names(&c, schema_name),
        #[cfg(feature = "postgres")]
        InferConnection::Pg(c) => super::information_schema::load_table_names(&c, schema_name),
        #[cfg(feature = "mysql")]
        InferConnection::Mysql(c) => super::information_schema::load_table_names(&c, schema_name),
    }
}

fn get_column_information(
    conn: &InferConnection,
    table: &TableName,
) -> Result<Vec<ColumnInformation>, Box<Error>> {
    let column_info = match *conn {
        #[cfg(feature = "sqlite")]
        InferConnection::Sqlite(ref c) => super::sqlite::get_table_data(c, table),
        #[cfg(feature = "postgres")]
        InferConnection::Pg(ref c) => super::information_schema::get_table_data(c, table),
        #[cfg(feature = "mysql")]
        InferConnection::Mysql(ref c) => super::information_schema::get_table_data(c, table),
    };
    if let Err(NotFound) = column_info {
        Err(format!("no table exists named {}", table.to_string()).into())
    } else {
        column_info.map_err(Into::into)
    }
}

fn determine_column_type(
    attr: &ColumnInformation,
    conn: &InferConnection,
) -> Result<ColumnType, Box<Error>> {
    match *conn {
        #[cfg(feature = "sqlite")]
        InferConnection::Sqlite(_) => super::sqlite::determine_column_type(attr),
        #[cfg(feature = "postgres")]
        InferConnection::Pg(_) => super::pg::determine_column_type(attr),
        #[cfg(feature = "mysql")]
        InferConnection::Mysql(_) => super::mysql::determine_column_type(attr),
    }
}

pub(crate) fn get_primary_keys(
    conn: &InferConnection,
    table: &TableName,
) -> Result<Vec<String>, Box<Error>> {
    let primary_keys: Vec<String> = try!(match *conn {
        #[cfg(feature = "sqlite")]
        InferConnection::Sqlite(ref c) => super::sqlite::get_primary_keys(c, table),
        #[cfg(feature = "postgres")]
        InferConnection::Pg(ref c) => super::information_schema::get_primary_keys(c, table),
        #[cfg(feature = "mysql")]
        InferConnection::Mysql(ref c) => super::information_schema::get_primary_keys(c, table),
    });
    if primary_keys.is_empty() {
        Err(format!(
            "Diesel only supports tables with primary keys. \
             Table {} has no primary key",
            table.to_string()
        ).into())
    } else if primary_keys.len() > 5 {
        Err(format!(
            "Diesel does not currently support tables with \
             primary keys consisting of more than 5 columns. \
             Table {} has {} columns in its primary key. \
             Please open an issue and we will increase the \
             limit.",
            table.to_string(),
            primary_keys.len()
        ).into())
    } else {
        Ok(primary_keys)
    }
}

pub fn load_foreign_key_constraints(
    database_url: &str,
    schema_name: Option<&str>,
) -> Result<Vec<ForeignKeyConstraint>, Box<Error>> {
    let connection = try!(InferConnection::establish(database_url));

    let constraints = match connection {
        #[cfg(feature = "sqlite")]
        InferConnection::Sqlite(c) => super::sqlite::load_foreign_key_constraints(&c, schema_name),
        #[cfg(feature = "postgres")]
        InferConnection::Pg(c) => {
            super::information_schema::load_foreign_key_constraints(&c, schema_name)
                .map_err(Into::into)
        }
        #[cfg(feature = "mysql")]
        InferConnection::Mysql(c) => {
            super::mysql::load_foreign_key_constraints(&c, schema_name).map_err(Into::into)
        }
    };

    constraints.map(|mut ct| {
        ct.sort();
        ct
    })
}

macro_rules! doc_comment {
    ($($token:tt)*) => {
        format!($($token)*)
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n")
    };
}

pub fn load_table_data(database_url: &str, name: TableName) -> Result<TableData, Box<Error>> {
    let connection = InferConnection::establish(database_url)?;
    let docs = doc_comment!(
        "Representation of the `{}` table.

        (Automatically generated by Diesel.)",
        name
    );
    let primary_key = get_primary_keys(&connection, &name)?;
    let primary_key = primary_key
        .iter()
        .map(|k| {
            if RESERVED_NAMES.contains(&k.as_str()) {
                format!("{}_", k)
            } else {
                k.clone()
            }
        })
        .collect();

    let column_data = get_column_information(&connection, &name)?
        .into_iter()
        .map(|c| {
            let ty = determine_column_type(&c, &connection)?;
            let rust_name = if RESERVED_NAMES.contains(&c.column_name.as_str()) {
                Some(format!("{}_", c.column_name))
            } else {
                None
            };

            Ok(ColumnDefinition {
                docs: doc_comment!(
                    "The `{}` column of the `{}` table.

                    Its SQL type is `{}`.

                    (Automatically generated by Diesel.)",
                    c.column_name,
                    name,
                    ty
                ),
                sql_name: c.column_name,
                ty,
                rust_name,
            })
        })
        .collect::<Result<_, Box<Error>>>()?;

    Ok(TableData {
        name,
        primary_key,
        column_data,
        docs,
    })
}