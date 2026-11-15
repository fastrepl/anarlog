import unittest

from prepare_api_service_secrets import (
    AI,
    BILLING,
    CORE,
    SHARED,
    SQLITECLOUD,
    SYNC,
    select,
)


class ServiceSecretsTests(unittest.TestCase):
    def setUp(self):
        self.api = [
            {"key": key, "value": "test-value"}
            for key in (SHARED | AI | CORE | SYNC) - {"ANARLOG_SQLITECLOUD_MODE"}
        ]
        self.webhooks = [
            {"key": key, "value": "webhook-value"}
            for key in {"DATABASE_URL", "STRIPE_WEBHOOK_SECRET"}
        ]
        self.api.append(
            {"key": "OTA_S3_SECRET_ACCESS_KEY", "value": "not-for-services"}
        )

    def test_roles_cannot_receive_unrelated_credentials(self):
        for role, excluded in [
            ("ai", BILLING | SYNC),
            ("sync", AI | BILLING),
            ("billing", AI | SYNC),
            ("core", AI),
        ]:
            with self.subTest(role=role):
                result = select(role, self.api, [], self.webhooks)
                self.assertFalse(result.keys() & excluded)
                self.assertNotIn("OTA_S3_SECRET_ACCESS_KEY", result)
                self.assertIn("SUPABASE_SERVICE_ROLE_KEY", result)

    def test_missing_or_conflicting_credentials_fail(self):
        with self.assertRaisesRegex(ValueError, "Missing"):
            select("billing", [], [])
        with self.assertRaisesRegex(ValueError, "Conflicting secret: SUPABASE_URL"):
            select("billing", self.api, [{"key": "SUPABASE_URL", "value": "different"}])

    def test_callback_origin_belongs_to_ai_service(self):
        self.assertEqual(
            select("ai", self.api, [])["API_BASE_URL"],
            "https://anarlog-inference.fly.dev",
        )
        self.assertEqual(select("all", self.api, [])["API_BASE_URL"], "test-value")

    def test_webhook_credentials_only_reach_billing(self):
        result = select("billing", self.api, [], self.webhooks)
        self.assertEqual(result["DATABASE_URL"], "webhook-value")
        self.assertEqual(result["LOOPS_API_KEY"], result["LOOPS_KEY"])
        for role in ["core", "gateway", "ai", "sync"]:
            result = select(role, self.api, [], self.webhooks)
            self.assertNotIn("DATABASE_URL", result)
            self.assertNotIn("STRIPE_WEBHOOK_SECRET", result)
        with self.assertRaisesRegex(ValueError, "DATABASE_URL"):
            select("billing", self.api, [])

    def test_slack_alert_webhooks_are_optional_and_billing_only(self):
        slack = [
            {
                "key": "SLACK_ALERT_ANARLOG_WEBHOOK_URL",
                "value": "https://hooks.example/a",
            },
            {"key": "SLACK_ALERT_CHAR_WEBHOOK_URL", "value": "https://hooks.example/c"},
        ]
        result = select("billing", self.api, [], self.webhooks + slack)
        self.assertEqual(
            result["SLACK_ALERT_ANARLOG_WEBHOOK_URL"], "https://hooks.example/a"
        )
        self.assertEqual(
            result["SLACK_ALERT_CHAR_WEBHOOK_URL"], "https://hooks.example/c"
        )
        without_slack = select("billing", self.api, [], self.webhooks)
        self.assertNotIn("SLACK_ALERT_ANARLOG_WEBHOOK_URL", without_slack)
        for role in ["core", "gateway", "ai", "sync"]:
            result = select(role, self.api, [], self.webhooks + slack)
            self.assertNotIn("SLACK_ALERT_ANARLOG_WEBHOOK_URL", result)
            self.assertNotIn("SLACK_ALERT_CHAR_WEBHOOK_URL", result)

    def test_sqlitecloud_mode_decides_which_provider_secrets_are_staged(self):
        def with_mode(mode):
            return self.api + [{"key": "ANARLOG_SQLITECLOUD_MODE", "value": mode}]

        supabase_only = [s for s in self.api if s["key"] not in SQLITECLOUD]
        for role in ["sync", "core", "gateway"]:
            with self.subTest(role=role):
                self.assertNotIn("ANARLOG_SQLITECLOUD_MODE", select(role, self.api, []))
                self.assertTrue(SQLITECLOUD <= select(role, self.api, []).keys())
                with self.assertRaisesRegex(ValueError, "SQLITECLOUD_PROJECT_URL"):
                    select(role, supabase_only, [])

                draining = select(role, with_mode("draining"), [])
                self.assertEqual(draining["ANARLOG_SQLITECLOUD_MODE"], "draining")
                self.assertTrue(SQLITECLOUD <= draining.keys())
                with self.assertRaisesRegex(ValueError, "SQLITECLOUD_PROJECT_URL"):
                    select(
                        role,
                        supabase_only
                        + [{"key": "ANARLOG_SQLITECLOUD_MODE", "value": "draining"}],
                        [],
                    )

                for secrets in [with_mode("retired"), with_mode(" retired ")]:
                    retired = select(role, secrets, [])
                    self.assertFalse(retired.keys() & SQLITECLOUD)
                    self.assertEqual(
                        retired["ANARLOG_SQLITECLOUD_MODE"].strip(), "retired"
                    )
                    self.assertIn("RESEND_API_KEY", retired)
                retired = select(
                    role,
                    supabase_only
                    + [{"key": "ANARLOG_SQLITECLOUD_MODE", "value": "retired"}],
                    [],
                )
                self.assertFalse(retired.keys() & SQLITECLOUD)

                with self.assertRaisesRegex(ValueError, "ANARLOG_SQLITECLOUD_MODE"):
                    select(role, with_mode("disabled"), [])

    def test_unknown_role_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "Unknown"):
            select("misspelled", self.api, [])

    def test_drive_callback_reaches_core_and_combined_roles(self):
        key = "GOOGLE_DRIVE_PICKER_REDIRECT_URI"
        url = "https://anarlog.so/app/google-drive-picker"
        secrets = [secret for secret in self.api if secret["key"] != key]
        for role in ["core", "all", "gateway", "legacy"]:
            with self.subTest(role=role):
                self.assertNotIn(key, select(role, secrets, []))
                self.assertEqual(
                    select(role, secrets + [{"key": key, "value": url}], [])[key],
                    url,
                )
        for role in ["ai", "sync", "billing"]:
            with self.subTest(role=role):
                self.assertNotIn(
                    key,
                    select(
                        role, secrets + [{"key": key, "value": url}], [], self.webhooks
                    ),
                )


if __name__ == "__main__":
    unittest.main()
