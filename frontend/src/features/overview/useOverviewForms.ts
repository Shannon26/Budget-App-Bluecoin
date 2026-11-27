import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { api } from "../../shared/api";
import { dateTimeValue, localDateValue } from "../../shared/date";
import { parseMinorUnits } from "../../shared/money";
import type { Account, BudgetState, TransactionKind } from "../../shared/types";
import { isAccountSelectable } from "../accounts/accountClassification";

interface SelectionState {
  initialized: boolean;
}

export function useOverviewForms(
  data: BudgetState,
  refresh: () => Promise<void>,
  onMessage: (message: string, error?: boolean) => void,
) {
  const [submitting, setSubmitting] = useState(false);

  const [accountName, setAccountName] = useState("");
  const [accountType, setAccountType] = useState("unclassified");
  const [accountNote, setAccountNote] = useState("");
  const [accountOpeningDate, setAccountOpeningDate] = useState("");
  const [accountHideFromSelection, setAccountHideFromSelection] = useState(false);
  const [accountHideFromReports, setAccountHideFromReports] = useState(false);
  const [openingBalance, setOpeningBalance] = useState("");

  const [transactionKind, setTransactionKind] = useState<TransactionKind>("expense");
  const [transactionName, setTransactionName] = useState("");
  const [transactionAccountId, setTransactionAccountIdState] = useState("");
  const transactionAccountSelection = useRef<SelectionState>({ initialized: false });
  const setTransactionAccountId = useCallback((value: string) => {
    transactionAccountSelection.current.initialized = true;
    setTransactionAccountIdState(value);
  }, []);
  const [transactionCategoryId, setTransactionCategoryId] = useState("");
  const [transactionAmount, setTransactionAmount] = useState("");
  const [transactionNote, setTransactionNote] = useState("");
  const [transactionDate, setTransactionDate] = useState(localDateValue());
  const [transactionReconciled, setTransactionReconciled] = useState(false);

  const [transferName, setTransferName] = useState("");
  const [transferFrom, setTransferFromState] = useState("");
  const transferFromSelection = useRef<SelectionState>({ initialized: false });
  const setTransferFrom = useCallback((value: string) => {
    transferFromSelection.current.initialized = true;
    setTransferFromState(value);
  }, []);
  const [transferTo, setTransferToState] = useState("");
  const transferToSelection = useRef<SelectionState>({ initialized: false });
  const setTransferTo = useCallback((value: string) => {
    transferToSelection.current.initialized = true;
    setTransferToState(value);
  }, []);
  const [transferAmount, setTransferAmount] = useState("");
  const [transferNote, setTransferNote] = useState("");
  const [transferDate, setTransferDate] = useState(localDateValue());
  const [transferReconciled, setTransferReconciled] = useState(false);

  const selectableAccounts = useMemo(
    () => data.accounts.filter(isAccountSelectable),
    [data.accounts],
  );

  useEffect(() => {
    if (transactionAccountId) {
      transactionAccountSelection.current.initialized = true;
      if (!selectableAccounts.some((account) => `${account.id}` === transactionAccountId)) {
        setTransactionAccountIdState("");
      }
      return;
    }
    if (transactionAccountSelection.current.initialized || selectableAccounts.length === 0) return;
    transactionAccountSelection.current.initialized = true;
    setTransactionAccountIdState(`${selectableAccounts[0].id}`);
  }, [selectableAccounts, transactionAccountId]);

  useEffect(() => {
    if (transferFrom) {
      transferFromSelection.current.initialized = true;
      if (!selectableAccounts.some((account) => `${account.id}` === transferFrom)) {
        setTransferFromState("");
      }
      return;
    }
    if (transferFromSelection.current.initialized || selectableAccounts.length === 0) return;
    transferFromSelection.current.initialized = true;
    setTransferFromState(`${selectableAccounts[0].id}`);
  }, [selectableAccounts, transferFrom]);

  useEffect(() => {
    if (transferTo) {
      transferToSelection.current.initialized = true;
      if (!selectableAccounts.some((account) => `${account.id}` === transferTo)) {
        setTransferToState("");
        return;
      }
      return;
    } else if (transferToSelection.current.initialized || !transferFrom) {
      return;
    }

    const alternative = selectableAccounts.find(
      (account) => `${account.id}` !== transferFrom,
    );
    if (!alternative) return;
    transferToSelection.current.initialized = true;
    setTransferToState(`${alternative.id}`);
  }, [selectableAccounts, transferFrom, transferTo]);

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
        account_type: accountType,
        note: accountNote.trim() ? accountNote : null,
        opening_date: accountOpeningDate || null,
        hide_from_selection: accountHideFromSelection || accountHideFromReports,
        hide_from_reports: accountHideFromReports,
      });
      setAccountName("");
      setAccountType("unclassified");
      setAccountNote("");
      setAccountOpeningDate("");
      setAccountHideFromSelection(false);
      setAccountHideFromReports(false);
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
    accountType,
    setAccountType,
    accountTypes: data.account_types,
    accountNote,
    setAccountNote,
    accountOpeningDate,
    setAccountOpeningDate,
    accountHideFromSelection,
    setAccountHideFromSelection,
    accountHideFromReports,
    setAccountHideFromReports,
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
