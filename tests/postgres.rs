use futures::TryStreamExt;
use sqlx::{postgres::PgConnection, Connection as _, Executor as _, Row as _};

#[cfg_attr(feature = "runtime-async-std", async_std::test)]
#[cfg_attr(feature = "runtime-tokio", tokio::test)]
async fn it_connects() -> anyhow::Result<()> {
    let mut conn = connect().await?;

    let row = sqlx::query("select 1 + 1").fetch_one(&mut conn).await?;

    assert_eq!(2, row.get(0));

    conn.close().await?;

    Ok(())
}

#[cfg_attr(feature = "runtime-async-std", async_std::test)]
#[cfg_attr(feature = "runtime-tokio", tokio::test)]
async fn it_executes() -> anyhow::Result<()> {
    let mut conn = connect().await?;

    let _ = conn
        .send(
            r#"
CREATE TEMPORARY TABLE users (id INTEGER PRIMARY KEY);
            "#,
        )
        .await?;

    for index in 1..=10_i32 {
        let cnt = sqlx::query("INSERT INTO users (id) VALUES ($1)")
            .bind(index)
            .execute(&mut conn)
            .await?;

        assert_eq!(cnt, 1);
    }

    let sum: i32 = sqlx::query("SELECT id FROM users")
        .fetch(&mut conn)
        .try_fold(
            0_i32,
            |acc, x| async move { Ok(acc + x.get::<i32, _>("id")) },
        )
        .await?;

    assert_eq!(sum, 55);

    Ok(())
}

#[cfg_attr(feature = "runtime-async-std", async_std::test)]
#[cfg_attr(feature = "runtime-tokio", tokio::test)]
async fn it_remains_stable_issue_30() -> anyhow::Result<()> {
    let mut conn = connect().await?;

    // This tests the internal buffer wrapping around at the end
    // Specifically: https://github.com/launchbadge/sqlx/issues/30

    let rows = sqlx::query("SELECT i, random()::text FROM generate_series(1, 1000) as i")
        .fetch_all(&mut conn)
        .await?;

    assert_eq!(rows.len(), 1000);
    assert_eq!(rows[rows.len() - 1].get::<i32, _>(0), 1000);

    Ok(())
}

#[cfg_attr(feature = "runtime-async-std", async_std::test)]
#[cfg_attr(feature = "runtime-tokio", tokio::test)]
async fn test_describe() -> anyhow::Result<()> {
    use sqlx::describe::Nullability::*;

    let mut conn = connect().await?;

    let _ = conn.send(r#"
        CREATE TEMP TABLE describe_test (
            id SERIAL primary key,
            name text not null,
            hash bytea
        )
    "#).await?;

    let describe = conn.describe("select nt.*, false from describe_test nt")
        .await?;

    assert_eq!(describe.result_columns[0].nullability, NonNull);
    assert_eq!(describe.result_columns[0].type_info.type_name(), "int4");
    assert_eq!(describe.result_columns[1].nullability, NonNull);
    assert_eq!(describe.result_columns[1].type_info.type_name(), "text");
    assert_eq!(describe.result_columns[2].nullability, Nullable);
    assert_eq!(describe.result_columns[2].type_info.type_name(), "bytea");
    assert_eq!(describe.result_columns[3].nullability, Unknown);
    assert_eq!(describe.result_columns[3].type_info.type_name(), "bool");

    Ok(())
}

async fn connect() -> anyhow::Result<PgConnection> {
    let _ = dotenv::dotenv();
    let _ = env_logger::try_init();
    Ok(PgConnection::open(dotenv::var("DATABASE_URL")?).await?)
}
