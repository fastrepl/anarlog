import unittest

from prepare_api_service_secrets import AI, BILLING, CORE, SHARED, SYNC, select


class ServiceSecretsTests(unittest.TestCase):
    def setUp(self):
        self.api = [
            {"key": key, "value": "test-value"} for key in SHARED | AI | CORE | SYNC
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
                result = select(role, self.api, [])
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

    def test_unknown_role_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "Unknown"):
            select("misspelled", self.api, [])


if __name__ == "__main__":
    unittest.main()
