//
//  cloudsync.h
//  cloudsync
//
//  Created by Marco Bambini on 16/05/24.
//

#ifndef __CLOUDSYNC__
#define __CLOUDSYNC__

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "database.h"
#include "block.h"

#ifdef __cplusplus
extern "C" {
#endif

#define CLOUDSYNC_VERSION                       "1.1.2"
#define CLOUDSYNC_MAX_TABLENAME_LEN             512

#define CLOUDSYNC_VALUE_NOTSET                  -1
#define CLOUDSYNC_TOMBSTONE_VALUE               "__[RIP]__"
#define CLOUDSYNC_RLS_RESTRICTED_VALUE          "__[RLS]__"
#define CLOUDSYNC_DISABLE_ROWIDONLY_TABLES      1
#define CLOUDSYNC_DEFAULT_ALGO                  "cls"
#define CLOUDSYNC_PAYLOAD_CHUNK_DEFAULT_SIZE    (5 * 1024 * 1024)
#define CLOUDSYNC_PAYLOAD_CHUNK_MIN_SIZE        (256 * 1024)
// Hard ceiling on the effective chunk size, regardless of the per-database
// payload_max_chunk_size setting. Protects the server (one chunk is built in
// memory and stored as a single artifact) and the tenant from a misconfigured
// value. Large TEXT/BLOB values still sync above this size: they are split
// across chunks by the fragment path. Only a row whose non-fragmentable
// scaffolding (primary key + column name + metadata, replicated into every
// fragment) exceeds the chunk size hits row_too_large, which is practically
// unreachable.
#define CLOUDSYNC_PAYLOAD_CHUNK_MAX_SIZE        (32 * 1024 * 1024)
#define CLOUDSYNC_PAYLOAD_CHUNK_SAFETY_MARGIN   (16 * 1024)
// Fragment sizing is a small fixpoint: after the first target estimate, only
// decimal metadata widths for part_index/part_count can change, so eight passes
// is ample while still preventing an accidental unbounded planning loop.
#define CLOUDSYNC_PAYLOAD_FRAGMENT_SIZE_FIXPOINT_ITERATIONS 8

// Machine-parseable error-code tokens. These prefix the human-readable text of
// permanent (non-retryable) failures so the CloudSync server can classify them
// from the error message alone — the only signal common to both the Postgres
// (pgconn.PgError.Message) and SQLite (result error text) backends. The server
// parses the bracketed code with /cloudsync_error\[([a-z0-9_]+)\]/ and decides
// retry policy; keep these strings stable and identical across backends. They
// carry a trailing ": " so they concatenate directly onto a message literal.
#define CLOUDSYNC_ERRCODE_PAYLOAD_TOO_LARGE     "cloudsync_error[payload_too_large]: "
#define CLOUDSYNC_ERRCODE_ROW_TOO_LARGE         "cloudsync_error[row_too_large]: "
#define CLOUDSYNC_ERRCODE_CHUNK_TOO_LARGE       "cloudsync_error[chunk_too_large]: "

#define CLOUDSYNC_CHANGES_NCOLS                 9

typedef enum {
    CLOUDSYNC_INIT_FLAG_NONE                        = 0,
    CLOUDSYNC_INIT_FLAG_SKIP_INT_PK_CHECK           = 1 << 0, // 1
    CLOUDSYNC_INIT_FLAG_SKIP_NOT_NULL_DEFAULT_CHECK = 1 << 1, // 2
    CLOUDSYNC_INIT_FLAG_SKIP_NOT_NULL_PRIKEYS_CHECK = 1 << 2  // 4
} CLOUDSYNC_INIT_FLAG;

// CRDT Algos
table_algo cloudsync_algo_from_name (const char *algo_name);
const char *cloudsync_algo_name (table_algo algo);

// Opaque structures
typedef struct cloudsync_payload_context cloudsync_payload_context;
typedef struct cloudsync_table_context cloudsync_table_context;

// CloudSync context
cloudsync_context *cloudsync_context_create (void *db);
const char *cloudsync_context_init (cloudsync_context *data);
void cloudsync_context_free (void *ctx);

// CloudSync global
int cloudsync_init_table (cloudsync_context *data, const char *table_name, const char *algo_name, CLOUDSYNC_INIT_FLAG init_flags);
int cloudsync_cleanup (cloudsync_context *data, const char *table_name);
int cloudsync_cleanup_all (cloudsync_context *data);
int cloudsync_terminate (cloudsync_context *data);
int cloudsync_insync (cloudsync_context *data);
int cloudsync_bumpseq (cloudsync_context *data);
void *cloudsync_siteid (cloudsync_context *data);
void cloudsync_reset_siteid (cloudsync_context *data);
void cloudsync_sync_key (cloudsync_context *data, const char *key, const char *value);
int64_t cloudsync_dbversion_next (cloudsync_context *data, int64_t merging_version);
int64_t cloudsync_dbversion (cloudsync_context *data);
void cloudsync_update_schema_hash (cloudsync_context *data);
int cloudsync_dbversion_check_uptodate (cloudsync_context *data);
bool cloudsync_config_exists (cloudsync_context *data);
bool cloudsync_context_is_initialized (cloudsync_context *data);
dbvm_t *cloudsync_colvalue_stmt (cloudsync_context *data, const char *tbl_name, bool *persistent);

// CloudSync alter table
int cloudsync_begin_alter (cloudsync_context *data, const char *table_name);
int cloudsync_commit_alter (cloudsync_context *data, const char *table_name);

// CloudSync getter/setter
void *cloudsync_db (cloudsync_context *data);
void *cloudsync_auxdata (cloudsync_context *data);
void cloudsync_set_auxdata (cloudsync_context *data, void *xdata);
int cloudsync_set_error (cloudsync_context *data, const char *err_user, int err_code);
int cloudsync_set_dberror (cloudsync_context *data);
const char *cloudsync_errmsg (cloudsync_context *data);
int cloudsync_errcode (cloudsync_context *data);
void cloudsync_reset_error (cloudsync_context *data);
int cloudsync_commit_hook (void *ctx);
void cloudsync_rollback_hook (void *ctx);
void cloudsync_set_schema (cloudsync_context *data, const char *schema);
const char *cloudsync_schema (cloudsync_context *data);
const char *cloudsync_table_schema (cloudsync_context *data, const char *table_name);

// Payload
// Receive-checkpoint modes for cloudsync_payload_apply's checkpoint_db_version
// argument. The receive cursor (check_dbversion/check_seq) must only ever land
// on a complete db_version boundary, otherwise a stop between chunks of a single
// source db_version silently skips the unapplied rows on the next /check (the
// server's cloudsync_payload_chunks uses db_version > since with no seq cursor).
//   >= 0                              advance the cursor to exactly this
//                                     (watermark_db_version), with checkpoint_seq.
//                                     Used once a chunk stream is fully applied.
//   CLOUDSYNC_CHECKPOINT_NONE         do not advance the cursor. Used for a
//                                     non-final chunk of a multi-chunk stream.
//   CLOUDSYNC_CHECKPOINT_LAST_APPLIED advance to this artifact's last applied
//                                     (db_version, seq). Legacy/monolithic
//                                     behavior: safe only for a complete payload
//                                     that ends on a db_version boundary.
#define CLOUDSYNC_CHECKPOINT_NONE          (-1)
#define CLOUDSYNC_CHECKPOINT_LAST_APPLIED  (-2)
int    cloudsync_payload_apply (cloudsync_context *data, const char *payload, int blen, int *nrows, int64_t checkpoint_db_version, int64_t checkpoint_seq);
int    cloudsync_payload_encode_step (cloudsync_payload_context *payload, cloudsync_context *data, int argc, dbvalue_t **argv);
int    cloudsync_payload_encode_final (cloudsync_payload_context *payload, cloudsync_context *data);
char  *cloudsync_payload_blob (cloudsync_payload_context *payload, int64_t *blob_size, int64_t *nrows);
size_t cloudsync_payload_context_size (size_t *header_size);
void   cloudsync_payload_context_free (cloudsync_payload_context *payload);
uint64_t cloudsync_payload_context_nrows (cloudsync_payload_context *payload);
size_t cloudsync_payload_context_bused (cloudsync_payload_context *payload);
int    cloudsync_payload_get (cloudsync_context *data, char **blob, int *blob_size, int *db_version, int64_t *new_db_version);
int    cloudsync_payload_save (cloudsync_context *data, const char *payload_path, int *blob_size); // available only on Desktop OS (no WASM, no mobile)
int    cloudsync_payload_max_chunk_size (cloudsync_context *data);
int    cloudsync_payload_encode_fragment_step (cloudsync_payload_context *payload, cloudsync_context *data,
                                               const char *tbl, int tbl_len,
                                               const void *pk, int pk_len,
                                               const char *col_name, int col_name_len,
                                               const void *fragment, int fragment_len,
                                               int64_t col_version, int64_t db_version,
                                               const void *site_id, int site_id_len,
                                               int64_t cl, int64_t seq,
                                               uint64_t value_checksum,
                                               int64_t total_size,
                                               int part_index, int part_count);
int    cloudsync_payload_fragment_target_size (cloudsync_context *data);
int    cloudsync_payload_fragment_count (int64_t total_size, int target_size);
int    cloudsync_payload_fragment_data_size (cloudsync_context *data,
                                             const char *tbl, int tbl_len,
                                             const void *pk, int pk_len,
                                             const char *col_name, int col_name_len,
                                             int64_t col_version, int64_t db_version,
                                             const void *site_id, int site_id_len,
                                             int64_t cl, int64_t seq,
                                             int64_t total_size,
                                             int part_index, int part_count);
uint64_t cloudsync_payload_encoded_value_checksum (dbvalue_t *value);
int    cloudsync_payload_encoded_value_header (dbvalue_t *value, char *header, int header_cap, int64_t *payload_len);

// CloudSync table context
int cloudsync_refill_metatable (cloudsync_context *data, const char *table_name);
int cloudsync_reset_metatable (cloudsync_context *data, const char *table_name);
cloudsync_table_context *table_lookup (cloudsync_context *data, const char *table_name);
void *table_column_lookup (cloudsync_table_context *table, const char *col_name, bool is_merge, int *index);
bool table_enabled (cloudsync_table_context *table);
void table_set_enabled (cloudsync_table_context *table, bool value);
bool table_add_to_context (cloudsync_context *data, table_algo algo, const char *table_name);
bool table_pk_exists (cloudsync_table_context *table, const char *value, size_t len);
int table_count_cols (cloudsync_table_context *table);
int table_count_pks (cloudsync_table_context *table);
const char *table_colname (cloudsync_table_context *table, int index);
char **table_pknames (cloudsync_table_context *table);
void table_set_pknames (cloudsync_table_context *table, char **pknames);
bool table_algo_isgos (cloudsync_table_context *table);
const char *table_schema (cloudsync_table_context *table);
int table_remove (cloudsync_context *data, cloudsync_table_context *table);
void table_free (cloudsync_table_context *table);

// Block-level LWW support
bool table_has_block_cols (cloudsync_table_context *table);
col_algo_t table_col_algo (cloudsync_table_context *table, int index);
const char *table_col_delimiter (cloudsync_table_context *table, int index);
int table_col_index (cloudsync_table_context *table, const char *col_name);
int block_materialize_column (cloudsync_context *data, cloudsync_table_context *table, const void *pk, int pklen, const char *base_col_name);
int cloudsync_setup_block_column (cloudsync_context *data, const char *table_name, const char *col_name, const char *delimiter, bool persist);

// Block column accessors (avoids accessing opaque struct from outside cloudsync.c)
dbvm_t *table_block_value_read_stmt (cloudsync_table_context *table);
dbvm_t *table_block_value_write_stmt (cloudsync_table_context *table);
dbvm_t *table_block_list_stmt (cloudsync_table_context *table);
const char *table_blocks_ref (cloudsync_table_context *table);
void table_set_col_delimiter (cloudsync_table_context *table, int col_idx, const char *delimiter);

// local merge/apply
int local_mark_insert_sentinel_meta (cloudsync_table_context *table, const void *pk, size_t pklen, int64_t db_version, int seq);
int local_update_sentinel (cloudsync_table_context *table, const void *pk, size_t pklen, int64_t db_version, int seq);
int local_mark_insert_or_update_meta (cloudsync_table_context *table, const void *pk, size_t pklen, const char *col_name, int64_t db_version, int seq);
int local_mark_delete_meta (cloudsync_table_context *table, const void *pk, size_t pklen, int64_t db_version, int seq);
int local_mark_delete_block_meta (cloudsync_table_context *table, const void *pk, size_t pklen, const char *block_colname, int64_t db_version, int seq);
int block_delete_value_external (cloudsync_context *data, cloudsync_table_context *table, const void *pk, size_t pklen, const char *block_colname);
int local_drop_meta (cloudsync_table_context *table, const void *pk, size_t pklen);
int local_update_move_meta (cloudsync_table_context *table, const void *pk, size_t pklen, const void *pk2, size_t pklen2, int64_t db_version);

// used by changes virtual table
int merge_insert_col (cloudsync_context *data, cloudsync_table_context *table, const void *pk, int pklen, const char *col_name, dbvalue_t *col_value, int64_t col_version, int64_t db_version, const char *site_id, int site_len, int64_t seq, int64_t *rowid);
int merge_insert (cloudsync_context *data, cloudsync_table_context *table, const char *insert_pk, int insert_pk_len, int64_t insert_cl, const char *insert_name, dbvalue_t *insert_value, int64_t insert_col_version, int64_t insert_db_version, const char *insert_site_id, int insert_site_id_len, int64_t insert_seq, int64_t *rowid);

// filter rewrite
char *cloudsync_filter_add_row_prefix(const char *filter, const char *prefix, char **columns, int ncols);

// decode bind context
char *cloudsync_pk_context_tbl (cloudsync_pk_decode_bind_context *ctx, int64_t *tbl_len);
void *cloudsync_pk_context_pk (cloudsync_pk_decode_bind_context *ctx, int64_t *pk_len);
char *cloudsync_pk_context_colname (cloudsync_pk_decode_bind_context *ctx, int64_t *colname_len);
int64_t cloudsync_pk_context_cl (cloudsync_pk_decode_bind_context *ctx);
int64_t cloudsync_pk_context_dbversion (cloudsync_pk_decode_bind_context *ctx);

#ifdef __cplusplus
}
#endif

#endif
