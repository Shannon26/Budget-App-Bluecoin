-- Align the existing Dining Out category with the observed Bluecoins taxonomy.
-- The guarded UPDATE changes only the parent pointer; historical entries keep their category_id.
UPDATE categories
SET parent_id = (
    SELECT id
    FROM categories
    WHERE name = 'Entertainment'
      AND kind = 'expense'
      AND parent_id IS NULL
      AND archived_at IS NULL
    ORDER BY id
    LIMIT 1
)
WHERE name = 'Dining Out'
  AND kind = 'expense'
  AND parent_id = (
      SELECT id
      FROM categories
      WHERE name = 'Food & Drink'
        AND kind = 'expense'
        AND parent_id IS NULL
      ORDER BY id
      LIMIT 1
  )
  AND EXISTS (
      SELECT 1
      FROM categories
      WHERE name = 'Entertainment'
        AND kind = 'expense'
        AND parent_id IS NULL
        AND archived_at IS NULL
  );
