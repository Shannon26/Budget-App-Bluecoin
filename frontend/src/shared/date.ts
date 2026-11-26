export function localDateValue(date = new Date()): string {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function dateTimeValue(date: string): string | undefined {
  return date ? `${date}T12:00:00` : undefined;
}
