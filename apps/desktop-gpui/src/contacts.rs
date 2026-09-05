//! Contacts tab data: `contacts/queries.ts` (humans, organizations, related
//! sessions, updates, pins, soft deletes) and the `@anlg/ui` avatar raster
//! (`packages/ui/src/lib/avatar.ts`).

use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Human {
    pub id: String,
    pub owner_user_id: String,
    pub created_at: String,
    pub organization_id: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub job_title: String,
    pub linkedin_username: String,
    pub memo: String,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub avatar_data_url: Option<String>,
    /// `metadata_json.contactSummary.facts`
    pub summary_facts: Vec<String>,
}

impl Human {
    /// `human.name || human.email || "Unnamed"`
    pub fn display_name(&self) -> String {
        if !self.name.is_empty() {
            self.name.clone()
        } else if !self.email.is_empty() {
            self.email.clone()
        } else {
            "Unnamed".to_string()
        }
    }

    /// `facehashName`
    pub fn avatar_seed(&self) -> String {
        if !self.name.is_empty() {
            self.name.clone()
        } else if !self.email.is_empty() {
            self.email.clone()
        } else {
            self.id.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organization {
    pub id: String,
    pub owner_user_id: String,
    pub created_at: String,
    pub name: String,
    pub memo: String,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub avatar_data_url: Option<String>,
}

/// `HumanSessionRecord`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanSession {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

const HUMANS_SQL: &str = "
  SELECT
    id,
    owner_user_id,
    created_at,
    organization_id,
    name,
    email,
    phone,
    job_title,
    linkedin_username,
    memo,
    pinned,
    pin_order,
    CASE
      WHEN json_valid(metadata_json)
      THEN json_extract(metadata_json, '$.avatarDataUrl')
    END AS avatar_data_url,
    CASE
      WHEN json_valid(metadata_json)
      THEN json_extract(metadata_json, '$.contactSummary')
    END AS contact_summary_json
  FROM humans
  WHERE deleted_at IS NULL
  ORDER BY name, email, id
";

const ORGANIZATIONS_SQL: &str = "
  SELECT id, owner_user_id, created_at, name, memo, pinned, pin_order,
    CASE
      WHEN json_valid(metadata_json)
      THEN json_extract(metadata_json, '$.avatarDataUrl')
    END AS avatar_data_url
  FROM organizations
  WHERE deleted_at IS NULL
  ORDER BY name, id
";

const HUMAN_SESSIONS_SQL: &str = "
  SELECT sessions.id, sessions.title, sessions.created_at
  FROM sessions
  WHERE sessions.deleted_at IS NULL
    AND EXISTS (
      SELECT 1
      FROM session_participants AS mapping
      WHERE mapping.session_id = sessions.id
        AND mapping.human_id = ?
        AND mapping.source <> 'excluded'
        AND mapping.deleted_at IS NULL
    )
  ORDER BY sessions.created_at DESC, sessions.id
";

/// `toggleContactPin`: the next `pin_order` spans humans and organizations.
const TOGGLE_PIN_SQL: &str = "
  UPDATE {table}
  SET
    pin_order = CASE
      WHEN pinned = 1 THEN NULL
      ELSE COALESCE((
        SELECT MAX(pin_order)
        FROM (
          SELECT pin_order FROM humans WHERE deleted_at IS NULL
          UNION ALL
          SELECT pin_order FROM organizations WHERE deleted_at IS NULL
        )
      ), 0) + 1
    END,
    pinned = CASE WHEN pinned = 1 THEN 0 ELSE 1 END,
    updated_at = ?
  WHERE id = ? AND deleted_at IS NULL
";

const CREATE_ORGANIZATION_SQL: &str = "
  INSERT INTO organizations (
    id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
    metadata_json, created_at, updated_at, deleted_at
  ) VALUES (
    ?, NULLIF((
      SELECT json_extract(value_json, '$.workspace_id')
      FROM app_settings
      WHERE id = 'cloudsync_workspace_binding'
    ), ''), COALESCE(
      NULLIF(NULLIF(?, ''), '00000000-0000-0000-0000-000000000000'),
      NULLIF((
        SELECT json_extract(value_json, '$.workspace_id')
        FROM app_settings
        WHERE id = 'cloudsync_workspace_binding'
      ), ''),
      '00000000-0000-0000-0000-000000000000'
    ), ?, '', 0, NULL, '{}', ?, ?, NULL
  )
";

fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

fn parse_summary_facts(json: Option<&str>) -> Vec<String> {
    let Some(json) = json else {
        return Vec::new();
    };
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(serde_json::Value::String(inner)) => serde_json::from_str(&inner).unwrap_or_default(),
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    value
        .get("facts")
        .and_then(|facts| facts.as_array())
        .map(|facts| {
            facts
                .iter()
                .filter_map(|fact| fact.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub async fn list_humans(pool: &SqlitePool) -> anyhow::Result<Vec<Human>> {
    type Row = (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        Option<i64>,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(HUMANS_SQL).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                owner_user_id,
                created_at,
                organization_id,
                name,
                email,
                phone,
                job_title,
                linkedin_username,
                memo,
                pinned,
                pin_order,
                avatar_data_url,
                summary,
            )| Human {
                id,
                owner_user_id,
                created_at,
                organization_id,
                name,
                email,
                phone,
                job_title,
                linkedin_username,
                memo,
                pinned: pinned != 0,
                pin_order,
                avatar_data_url: avatar_data_url.filter(|url| !url.is_empty()),
                summary_facts: parse_summary_facts(summary.as_deref()),
            },
        )
        .collect())
}

pub async fn list_organizations(pool: &SqlitePool) -> anyhow::Result<Vec<Organization>> {
    type Row = (
        String,
        String,
        String,
        String,
        String,
        i64,
        Option<i64>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(ORGANIZATIONS_SQL).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, owner_user_id, created_at, name, memo, pinned, pin_order, avatar_data_url)| {
                Organization {
                    id,
                    owner_user_id,
                    created_at,
                    name,
                    memo,
                    pinned: pinned != 0,
                    pin_order,
                    avatar_data_url: avatar_data_url.filter(|url| !url.is_empty()),
                }
            },
        )
        .collect())
}

pub async fn human_sessions(
    pool: &SqlitePool,
    human_id: &str,
) -> anyhow::Result<Vec<HumanSession>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(HUMAN_SESSIONS_SQL)
        .bind(human_id)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(id, title, created_at)| HumanSession {
            id,
            title,
            created_at,
        })
        .collect())
}

/// `updateHuman` for one column.
pub async fn update_human_field(
    pool: &SqlitePool,
    human_id: &str,
    column: &'static str,
    value: &str,
) -> anyhow::Result<()> {
    let sql = match column {
        "name" => "UPDATE humans SET name = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
        "email" => {
            "UPDATE humans SET email = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        }
        "phone" => {
            "UPDATE humans SET phone = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        }
        "job_title" => {
            "UPDATE humans SET job_title = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        }
        "linkedin_username" => {
            "UPDATE humans SET linkedin_username = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        }
        "memo" => "UPDATE humans SET memo = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
        "organization_id" => {
            "UPDATE humans SET organization_id = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        }
        other => anyhow::bail!("unknown human column {other}"),
    };
    sqlx::query(sql)
        .bind(value)
        .bind(now_iso())
        .bind(human_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `softDeleteContact`
pub async fn soft_delete(pool: &SqlitePool, table: &'static str, id: &str) -> anyhow::Result<()> {
    let sql = match table {
        "humans" => {
            "UPDATE humans SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        }
        "organizations" => {
            "UPDATE organizations SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        }
        other => anyhow::bail!("unknown contact table {other}"),
    };
    let now = now_iso();
    sqlx::query(sql)
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `toggleContactPin`
pub async fn toggle_pin(pool: &SqlitePool, table: &'static str, id: &str) -> anyhow::Result<()> {
    if !matches!(table, "humans" | "organizations") {
        anyhow::bail!("unknown contact table {table}");
    }
    let sql = TOGGLE_PIN_SQL.replace("{table}", table);
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(now_iso())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `createOrganization`
pub async fn create_organization(pool: &SqlitePool, name: &str) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();
    sqlx::query(CREATE_ORGANIZATION_SQL)
        .bind(&id)
        .bind("00000000-0000-0000-0000-000000000000")
        .bind(name)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    Ok(id)
}

// --- `packages/ui/src/lib/avatar.ts` ---

pub type Rgb = [f64; 3];

const BAYER_4X4: [f64; 16] = [
    0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0,
];

/// FNV-1a over the NFKC code points, like `hashString`.
fn hash_string(value: &str) -> u32 {
    use unicode_normalization::UnicodeNormalization as _;
    let mut hash: u32 = 2166136261;
    for character in value.nfkc() {
        hash ^= character as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

/// `mulberry32`
struct Mulberry32(u32);

impl Mulberry32 {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut value = self.0;
        value = (value ^ (value >> 15)).wrapping_mul(value | 1);
        value ^= value.wrapping_add((value ^ (value >> 7)).wrapping_mul(value | 61));
        f64::from(value ^ (value >> 14)) / 4_294_967_296.0
    }
}

fn clamp(value: f64, minimum: f64, maximum: f64) -> f64 {
    value.max(minimum).min(maximum)
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> Rgb {
    let s = saturation / 100.0;
    let l = lightness / 100.0;
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let segment = ((hue % 360.0) + 360.0) % 360.0 / 60.0;
    let secondary = chroma * (1.0 - ((segment % 2.0) - 1.0).abs());
    let (red, green, blue) = if segment < 1.0 {
        (chroma, secondary, 0.0)
    } else if segment < 2.0 {
        (secondary, chroma, 0.0)
    } else if segment < 3.0 {
        (0.0, chroma, secondary)
    } else if segment < 4.0 {
        (0.0, secondary, chroma)
    } else if segment < 5.0 {
        (secondary, 0.0, chroma)
    } else {
        (chroma, 0.0, secondary)
    };
    let m = l - chroma / 2.0;
    [(red + m) * 255.0, (green + m) * 255.0, (blue + m) * 255.0]
}

fn create_palette(random: &mut Mulberry32, color_count: usize) -> Vec<Rgb> {
    let count = color_count.clamp(2, 5);
    let base_hue = random.next() * 360.0;
    let spreads = [32.0, 52.0, 138.0, 208.0];
    let spread = spreads[((random.next() * spreads.len() as f64).floor() as usize).min(3)];
    (0..count)
        .map(|index| {
            let hue = (base_hue + spread * index as f64 + (random.next() - 0.5) * 18.0) % 360.0;
            let saturation = 58.0 + random.next() * 24.0;
            let lightness = 48.0 + random.next() * 24.0;
            hsl_to_rgb(hue, saturation, lightness)
        })
        .collect()
}

struct Sphere {
    x: f64,
    y: f64,
    radius: f64,
    color: Rgb,
}

fn create_spheres(random: &mut Mulberry32, colors: &[Rgb], count: usize) -> Vec<Sphere> {
    (0..count.clamp(1, 7))
        .map(|index| Sphere {
            x: -0.1 + random.next() * 1.2,
            y: -0.1 + random.next() * 1.2,
            radius: 0.24 + random.next() * 0.42,
            color: colors[(index + 1) % colors.len()],
        })
        .collect()
}

fn interpolate_palette(colors: &[Rgb], position: f64) -> Rgb {
    let scaled = position * (colors.len() - 1) as f64;
    let left_index = scaled.floor() as usize;
    let right_index = (left_index + 1).min(colors.len() - 1);
    let amount = scaled - left_index as f64;
    let left = colors[left_index.min(colors.len() - 1)];
    let right = colors[right_index];
    [
        left[0] + (right[0] - left[0]) * amount,
        left[1] + (right[1] - left[1]) * amount,
        left[2] + (right[2] - left[2]) * amount,
    ]
}

fn blend_spheres(base: Rgb, x: f64, y: f64, spheres: &[Sphere]) -> Rgb {
    let mut color = base;
    for sphere in spheres {
        let distance_squared = (x - sphere.x).powi(2) + (y - sphere.y).powi(2);
        let influence = (-distance_squared / (2.0 * sphere.radius.powi(2))).exp() * 0.72;
        for (channel, value) in color.iter_mut().enumerate() {
            *value += (sphere.color[channel] - *value) * influence;
        }
    }
    color
}

fn quantize(value: f64, steps: f64) -> f64 {
    clamp(
        (clamp(value, 0.0, 255.0) / 255.0 * steps).round() * (255.0 / steps),
        0.0,
        255.0,
    )
}

/// `createAvatarPixels` with the app's recipe (4 colours, 4 spheres, 0.3
/// dither, dithered): RGBA bytes, row-major, `size × size`.
pub fn avatar_pixels(seed: &str, size: usize) -> Vec<u8> {
    let dimension = size.max(1);
    let mut pixels = vec![0u8; dimension * dimension * 4];
    let mut random = Mulberry32(hash_string(seed));
    let colors = create_palette(&mut random, 4);
    let angle = random.next() * std::f64::consts::PI * 2.0;
    let spheres = create_spheres(&mut random, &colors, 4);
    let steps = 6.0;
    for y in 0..dimension {
        for x in 0..dimension {
            let nx = (x as f64 + 0.5) / dimension as f64;
            let ny = (y as f64 + 0.5) / dimension as f64;
            let directional = clamp(
                0.5 + (nx - 0.5) * angle.cos() + (ny - 0.5) * angle.sin(),
                0.0,
                1.0,
            );
            let base = interpolate_palette(&colors, directional);
            let color = blend_spheres(base, nx, ny, &spheres);
            let threshold = (BAYER_4X4[(y % 4) * 4 + (x % 4)] / 16.0 - 0.5) * 255.0;
            let index = (y * dimension + x) * 4;
            for channel in 0..3 {
                pixels[index + channel] = quantize(color[channel] + threshold * 0.3, steps) as u8;
            }
            pixels[index + 3] = 255;
        }
    }
    pixels
}

/// `avatarInitials`
pub fn avatar_initials(value: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    value
        .split_whitespace()
        .map(|part| {
            part.nfkc()
                .filter(|c| c.is_alphanumeric())
                .collect::<Vec<char>>()
        })
        .filter(|part| !part.is_empty())
        .take(2)
        .map(|part| part[0].to_uppercase().collect::<String>())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_the_javascript_fnv() {
        // `hashString` in the web app for the same seeds.
        assert_eq!(hash_string("Ada"), 2596513115);
        assert_eq!(hash_string("john@example.com"), 152767649);
    }

    #[test]
    fn mulberry32_matches_the_javascript_sequence() {
        // mulberry32(1): first two outputs from the reference implementation.
        let mut random = Mulberry32(1);
        let first = random.next();
        let second = random.next();
        assert!((first - 0.6270739405881613).abs() < 1e-12, "{first}");
        assert!((second - 0.002735721180215478).abs() < 1e-12, "{second}");
        // Seeded with `hashString("Ada")`.
        let mut random = Mulberry32(hash_string("Ada"));
        assert!((random.next() - 0.26034449180588126).abs() < 1e-12);
        assert!((random.next() - 0.010317299980670214).abs() < 1e-12);
        assert!((random.next() - 0.7853058404289186).abs() < 1e-12);
    }

    #[test]
    fn pixels_are_opaque_and_deterministic() {
        let a = avatar_pixels("Ada", 8);
        let b = avatar_pixels("Ada", 8);
        assert_eq!(a, b);
        assert_eq!(a.len(), 8 * 8 * 4);
        assert!(a.chunks(4).all(|px| px[3] == 255));
        assert_ne!(a, avatar_pixels("Eve", 8));
    }

    #[test]
    fn initials_take_the_first_two_words() {
        assert_eq!(avatar_initials("ada lovelace"), "AL");
        assert_eq!(avatar_initials("  john@example.com "), "J");
        assert_eq!(avatar_initials("Élodie   d'Arc"), "ÉD");
    }
}
