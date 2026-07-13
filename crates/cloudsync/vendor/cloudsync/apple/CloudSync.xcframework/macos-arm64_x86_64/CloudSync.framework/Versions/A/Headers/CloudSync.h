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

#define CLOUDSYNC_VERSION                       "1.0.20"
#define CLOUDSYNC_MAX_TABLENAME_LEN             512

#define CLOUDSYNC_VALUE_NOTSET                  -1
#define CLOUDSYNC_TOMBSTONE_VALUE               "__[RIP]__"
#define CLOUDSYNC_RLS_RESTRICTED_VALUE          "__[RLS]__"
#define CLOUDSYNC_DISABLE_ROWIDONLY_TABLES      1
#define CLOUDSYNC_DEFAULT_ALGO                  "cls"

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
int    cloudsync_payload_apply (cloudsync_context *data, const char *payload, int blen, int *nrows);
int    cloudsync_payload_encode_step (cloudsync_payload_context *payload, cloudsync_context *data, int argc, dbvalue_t **argv);
int    cloudsync_payload_encode_final (cloudsync_payload_context *payload, cloudsync_context *data);
char  *cloudsync_payload_blob (cloudsync_payload_context *payload, int64_t *blob_size, int64_t *nrows);
size_t cloudsync_payload_context_size (size_t *header_size);
int    cloudsync_payload_get (cloudsync_context *data, char **blob, int *blob_size, int *db_version, int64_t *new_db_version);
int    cloudsync_payload_save (cloudsync_context *data, const char *payload_path, int *blob_size); // available only on Desktop OS (no WASM, no mobile)

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
