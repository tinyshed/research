import unittest

import numpy as np

import joint


class HarnessChecks(unittest.TestCase):
    def test_family_never_crosses_a_fold(self):
        families = ["a", "a", "b", "c", "d", "e", "f"]
        visited = np.zeros((3, len(families)), dtype=int)
        for draw, _, train, test in joint.folds(families):
            self.assertFalse(set(np.asarray(families)[train]) & set(np.asarray(families)[test]))
            visited[draw, test] += 1
        np.testing.assert_array_equal(visited, np.ones_like(visited))

    def test_normalisation_does_not_read_test_statistics(self):
        a, b, mean, scale = joint.normalise([[0.0, np.nan], [2.0, np.nan]], [[1000.0, 99.0]])
        np.testing.assert_allclose(mean, [1, 0])
        np.testing.assert_allclose(a[:, 0], [-1, 1])
        self.assertEqual(b[0, 0], 10)
        self.assertTrue(np.isfinite(scale).all())

    def test_semantic_slots_are_not_collapsed_to_one_number(self):
        vectors = {"price": np.array([1.0, 2.0]), "id": np.array([-1.0, -2.0])}
        lex = {"price": [0, 1, 0, 0, 0, 0, 0, 0, 1], "id": [1, 0, 0, 0, 0, 0, 0, 0, 0]}
        a = joint.semantic({"measure": ["column:price"]}, lex, vectors)
        b = joint.semantic({"by": ["price"]}, lex, vectors)
        self.assertFalse(np.array_equal(a, b, equal_nan=True))
        self.assertEqual(len(a), 4 * (9 + 2 + 1))

    def test_profile_handles_large_public_numeric_strings_and_empty_results(self):
        p = {"rows": [["A", "11182550283109333911"], ["B", None]]}
        values = joint.profile(p, {"a"})
        self.assertEqual(len(values), len(joint.PROFILE_NAMES))
        self.assertGreater(values[0], 18)
        self.assertEqual(values[4], 20)
        self.assertEqual(len(joint.profile({"rows": []}, set())), len(values))

    def test_joint_features_retain_interactions(self):
        base = np.ones((2, 16))
        sem = np.array([[1.0, 0.0], [0.0, 1.0]])
        prof = np.array([[2.0], [3.0]])
        out = joint.joint_matrix(base, sem, prof)
        self.assertEqual(out.shape, (2, 2 * (16 + 2 + 1) + 2 * 17))
        self.assertEqual(out[0, 38 + 16], 2)
        self.assertEqual(out[1, -1], 3)

    def test_linear_packing_preserves_missing_and_clipped_values(self):
        model = {"coefficients": [0.2, -0.5], "intercept": 0.1, "mean": [2, 4], "scale": [1, 2]}
        blob = joint.pack_linear(model)
        values = joint.predict_linear(blob, [[2, 4], [np.nan, 8], [1000, np.nan]])
        expected = 1 / (1 + np.exp(-np.array([0.1, -0.9, 2.1])))
        np.testing.assert_allclose(values, expected, atol=1e-7)
        with self.assertRaises(ValueError):
            joint.predict_linear(blob[:-1], [[0, 0]])

    def test_fractional_labels_are_not_rounded(self):
        X = np.zeros((12, 3))
        y = np.full(12, 0.2)
        train = np.arange(12) < 10
        probabilities, model = joint.fit_predict("joint", X, y, train, ~train, joint.SEED)
        np.testing.assert_allclose(probabilities, 0.2, atol=1e-3)
        np.testing.assert_allclose(joint.predict_linear(joint.pack_linear(model), X[~train]),
                                   probabilities, atol=1e-7)

    def test_identity_sets_have_a_canonical_json_representation(self):
        self.assertEqual(joint.clean_json((1, ("sum", frozenset({(2, 3), (1, 4)})), None, None)),
                         [1, ["sum", [[1, 4], [2, 3]]], None, None])


if __name__ == "__main__":
    unittest.main()
