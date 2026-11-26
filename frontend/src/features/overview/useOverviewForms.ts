import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { api } from "../../shared/api";
import { dateTimeValue, localDateValue } from "../../shared/date";
import { parseMinorUnits } from "../../shared/money";
import type { BudgetState, TransactionKind } from "../../shared/types";

export function useOverviewForms(
  data: BudgetState,
  refresh: () => Promise<void>,
  onMessage: (message: string, error?: boolean) => void,
) {
  const [submitting, setSubmitting] = useState(false);

  const [accountName, setAccountName] = useState("");
  const [openingBalance, setOpeningBalance] = useState("");

  const [transactionKind, setTransactionKind] = useState<TransactionKind>("expense");
  const [transactionName, setTransactionName] = useState("");
  const [transactionAccountId, setTransactionAccountId] = useState("");
  const [transactionCategoryId, setTransactionCategoryId] = useState("");
  const [transactionAmount, setTransactionAmount] = useState("");
  const [transactionNote, setTransactionNote] = useState("");
  const [transactionDate, setTransactionDate] = useState(localDateValue());
  const [transactionReconciled, setTransactionReconciled] = useState(false);

  const [transferName, setTransferName] = useState("");
  const [transferFrom, setTransferFrom] = useState("");
  const [transferTo, setTransferTo] = useState("");
  const [transferAmount, setTransferAmount] = useState("");
  const [transferNote, setTransferNote] = useState("");
  const [transferDate, setTransferDate] = useState(localDateValue());
  const [transferReconciled, setTransferReconciled] = useState(false);

  useEffect(() => {
    setTransactionAccountId((current) => current || `${data.accounts[0]?.id ?? ""}`);
    setTransferFrom((current) => current || `${data.accounts[0]?.id ?? ""}`);
    setTransferTo(
      (current) => current || `${data.accounts[1]?.id ?? data.accounts[0]?.id ?? ""}`,
    );
  }, [data.accounts]);

  const transactionCategories = useMemo(
    () =>
      data.categories.filter(
        (category) =>
          category.kind === transactionKind && category.archived_at === null,
      ),
    [data.categories, transactionKind],
  );

  async function handleCreateAccount(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    onMessage("", true);
    try {
      const openingMinor = parseMinorUnits(openingBalance || "0", true);
      if (openingMinor === null) throw new Error("Enter a valid opening balance");
      await api.createAccount({
        name: accountName,
        currency: "INR",
        opening_balance_minor: openingMinor,
      });
      setAccountName("");
      setOpeningBalance("");
      onMessage("Account created. Its opening balance is now part of the ledger.");
      await refresh();
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not create account", true);
    } finally {
      setSubmitting(false);
    }
  }

  async function handleTransaction(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    onMessage("", true);
    try {
      const amountMinor = parseMinorUnits(transactionAmount);
      if (amountMinor === null || amountMinor <= 0) {
        throw new Error("Enter an amount greater than zero, with at most two decimal places");
      }
      if (!transactionAccountId) throw new Error("Choose an account");
      await api.createTransaction({
        kind: transactionKind,
        account_id: Number(transactionAccountId),
        amount_minor: amountMinor,
        name: transactionName,
        category_id: transactionCategoryId ? Number(transactionCategoryId) : null,
        reconciled: transactionReconciled,
        note: transactionNote,
        occurred_at: dateTimeValue(transactionDate),
      });
      setTransactionName("");
      setTransactionAmount("");
      setTransactionNote("");
      setTransactionReconciled(false);
      onMessage(`${transactionKind === "income" ? "Income" : "Expense"} recorded.`);
      await refresh();
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not record transaction", true);
    } finally {
      setSubmitting(false);
    }
  }

  async function handleTransfer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    onMessage("", true);
    try {
      const amountMinor = parseMinorUnits(transferAmount);
      if (amountMinor === null || amountMinor <= 0) {
        throw new Error("Enter a transfer amount greater than zero");
      }
      if (!transferFrom || !transferTo) throw new Error("Choose both accounts");
      if (transferFrom === transferTo) throw new Error("Choose two different accounts");
      await api.createTransfer({
        from_account_id: Number(transferFrom),
        to_account_id: Number(transferTo),
        amount_minor: amountMinor,
        name: transferName,
        reconciled: transferReconciled,
        note: transferNote,
        occurred_at: dateTimeValue(transferDate),
      });
      setTransferName("");
      setTransferAmount("");
      setTransferNote("");
      setTransferReconciled(false);
      onMessage("Transfer complete. Both balances updated; income and expenses did not change.");
      await refresh();
    } catch (reason) {
      onMessage(reason instanceof Error ? reason.message : "Could not complete transfer", true);
    } finally {
      setSubmitting(false);
    }
  }

  return {
    submitting,
    accountName,
    setAccountName,
    openingBalance,
    setOpeningBalance,
    handleCreateAccount,
    transactionKind,
    setTransactionKind,
    transactionName,
    setTransactionName,
    transactionAccountId,
    setTransactionAccountId,
    transactionCategoryId,
    setTransactionCategoryId,
    transactionAmount,
    setTransactionAmount,
    transactionNote,
    setTransactionNote,
    transactionDate,
    setTransactionDate,
    transactionReconciled,
    setTransactionReconciled,
    transactionCategories,
    handleTransaction,
    transferName,
    setTransferName,
    transferFrom,
    setTransferFrom,
    transferTo,
    setTransferTo,
    transferAmount,
    setTransferAmount,
    transferNote,
    setTransferNote,
    transferDate,
    setTransferDate,
    transferReconciled,
    setTransferReconciled,
    handleTransfer,
  };
}

export type OverviewForms = ReturnType<typeof useOverviewForms>;
