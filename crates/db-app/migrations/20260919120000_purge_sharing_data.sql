-- Le partage et les espaces de travail ont été retirés de l'application. Les
-- tables restent en place : les migrations livrées sont immuables, et une
-- version antérieure doit continuer à ouvrir cette base sans erreur. Seules
-- les lignes partent.
--
-- Additive au sens de la règle downgrade-safe : aucune colonne ni table n'est
-- supprimée, donc un build plus ancien retrouve le schéma qu'il attend, avec
-- des tables vides.
DELETE FROM shared_session_attachment_cache;
DELETE FROM shared_session_cache;
DELETE FROM session_share_activation;
DELETE FROM session_share_sync_state;
DELETE FROM workspace_memberships;
DELETE FROM cloudsync_writable_workspaces;
DELETE FROM workspaces;
