import pg from "pg";

import { env } from "../../env";

interface ColumnInfo {
  table_schema: string;
  table_name: string;
  column_name: string;
  data_type: string;
}

let cachedSchema: string | null = null;

export async function fetchDatabaseSchema(): Promise<string> {
  if (cachedSchema) {
    return cachedSchema;
  }

  const client = new pg.Client(env.DATABASE_URL);
  await client.connect();

  try {
    const { rows } = await client.query<ColumnInfo>(`
      SELECT table_schema, table_name, column_name, data_type
      FROM information_schema.columns
      WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
      ORDER BY table_schema, table_name, ordinal_position
    `);

    cachedSchema = formatSchema(rows);
    return cachedSchema;
  } finally {
    await client.end();
  }
}

function formatSchema(rows: ColumnInfo[]): string {
  const tables = new Map<string, string[]>();

  for (const row of rows) {
    const key = `${row.table_schema}.${row.table_name}`;
    if (!tables.has(key)) {
      tables.set(key, []);
    }
    tables.get(key)!.push(`${row.column_name}: ${row.data_type}`);
  }

  const lines: string[] = [];
  for (const [table, cols] of tables) {
    lines.push(`${table}(${cols.join(", ")})`);
  }

  return lines.join("\n");
}
