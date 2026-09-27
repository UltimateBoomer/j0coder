import unittest

from ci_changes import ALL, checks_for


class ChangesTest(unittest.TestCase):
    def test_docs_only(self):
        self.assertEqual(checks_for("docs/testing.md"), set())
        self.assertEqual(checks_for("README.md"), set())

    def test_backend_changes(self):
        self.assertEqual(checks_for("crates/platform/src/api.rs"),
                         {"rust", "integration", "app_image"})
        self.assertEqual(checks_for("migrations/001.sql"),
                         {"rust", "integration", "app_image"})

    def test_web_changes(self):
        self.assertEqual(checks_for("web/src/App.svelte"),
                         {"web", "integration", "app_image"})
        self.assertEqual(checks_for("tests/browser/routing.spec.ts"),
                         {"web", "integration"})

    def test_deployment_changes(self):
        self.assertEqual(checks_for("deploy/helm/locoder/values.yaml"), {"helm"})
        self.assertEqual(checks_for("deploy/local-kubernetes/lima.yaml"), {"helm", "config"})
        self.assertEqual(checks_for("compose.yaml"), {"config"})
        self.assertEqual(checks_for("deploy/nginx.conf"), {"config"})
        self.assertEqual(checks_for("deploy/Toolchain.Containerfile"), {"toolchain_image"})

    def test_shared_and_unknown_changes(self):
        self.assertEqual(checks_for(".dockerignore"), {"app_image", "toolchain_image"})
        self.assertEqual(checks_for(".github/workflows/publish-images.yml"),
                         {"app_image", "toolchain_image"})
        self.assertEqual(checks_for(".github/workflows/ci.yml"), ALL)
        self.assertEqual(checks_for("some-new-file"), ALL)


if __name__ == "__main__":
    unittest.main()
