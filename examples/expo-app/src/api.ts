export async function fetchUser(id: number): Promise<{ id: number; name: string }> {
  // In real code this would fetch from an API
  throw new Error('Not implemented — should be mocked in tests');
}

export async function saveUser(user: { id: number; name: string }): Promise<void> {
  throw new Error('Not implemented — should be mocked in tests');
}
