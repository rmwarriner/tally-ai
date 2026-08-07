//! Synthetic QIF fixtures for tests. Mirrors the shape of real Banktivity
//! exports (multi-account, splits, transfers, investment cash flows, custom
//! 401k type) without checking in real financial data.

pub fn banktivity_minimal() -> &'static str {
    concat!(
        "!Option:AutoSwitch\n",
        "!Account\n",
        // Balances declared here equal the net of each account's transactions
        // below (matching how real Banktivity exports behave).
        "NTestChecking\nTBank\nB1050.00\n^\n",
        "NTestCard\nTCCard\nB-80.00\n^\n",
        "NTestSavings\nTBank\nB600.00\n^\n",
        "!Clear:AutoSwitch\n",
        "!Type:Cat\n",
        "NNeeds:Groceries\nE\n^\n",
        "NEmployment Income:Salary\nI\n^\n",
        "!Account\n",
        "NTestChecking\nTBank\n^\n",
        "!Type:Bank\n",
        // Opening balance.
        "D1/1/26\nT1000.00\nCc\nPSTARTING BALANCE\nMBALANCE ADJUSTMENT\n^\n",
        // Normal expense.
        "D1/4/26\nT-50.00\nCc\nPGrocer\nLNeeds:Groceries\nMWeekly shop\n^\n",
        // Transfer to savings.
        "D1/5/26\nT-100.00\nCc\nPTransfer\nL[TestSavings]\n^\n",
        // Income.
        "D1/10/26\nT200.00\nCc\nPEmployer\nLEmployment Income:Salary\n^\n",
        "!Account\n",
        "NTestCard\nTCCard\n^\n",
        "!Type:CCard\n",
        "D1/1/26\nT-50.00\nCc\nPSTARTING BALANCE\nMBALANCE ADJUSTMENT\n^\n",
        // Split.
        "D1/20/26\nT-30.00\nCc\nPAmazon\n",
        "EItemA\nSNeeds:Groceries\n$-20.00\n",
        "EItemB\nSNeeds:Groceries\n$-10.00\n",
        "^\n",
        "!Account\n",
        "NTestSavings\nTBank\n^\n",
        "!Type:Bank\n",
        "D1/1/26\nT500.00\nCc\nPSTARTING BALANCE\nMBALANCE ADJUSTMENT\n^\n",
        // Receiving end of the transfer.
        "D1/5/26\nT100.00\nCc\nPTransfer\nL[TestChecking]\n^\n",
    )
}

#[allow(dead_code)]
pub fn with_investment_cash_flow() -> &'static str {
    concat!(
        "!Account\nNRetirement\nTInvst\n^\n",
        "!Type:Invst\n",
        // Security trade — should be skipped + counted.
        "D1/1/26\nNBuy\nYBRK Class B\nI67.23\nQ245.673\nT16,516.60\nO0.00\nCc\n^\n",
        // Cash flow (paycheck contribution) — should be kept as transfer.
        "D1/7/26\nNCash\nT147.56\nO0.00\nCc\nPEmployer Payroll\nMContribution\nL[Checking]\n^\n",
    )
}
