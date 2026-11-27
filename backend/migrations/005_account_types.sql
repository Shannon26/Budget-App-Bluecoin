PRAGMA foreign_keys = ON;

ALTER TABLE accounts
    ADD COLUMN account_type TEXT NOT NULL DEFAULT 'unclassified'
    CHECK (
        account_type IN (
            'unclassified',
            'cash',
            'checking',
            'savings',
            'wallet',
            'investment',
            'cryptocurrency',
            'property',
            'receivable',
            'other_asset',
            'credit_card',
            'loan',
            'mortgage',
            'payable',
            'other_liability'
        )
    );
