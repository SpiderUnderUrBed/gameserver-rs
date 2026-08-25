import { expect, test } from 'vitest'
import { fileBrowserStore } from '../src/lib/stores/fileBrowserStore.svelte'
// import { sum } from './sum.js'

// export function sum(a: number, b: number) {
//   return a + b
// }

// test('adds 1 + 2 to equal 3', () => {
//   expect(sum(1, 2)).toBe(3)
// })

await fetch('/api/signin', {
  method: 'POST',
  headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
  body: new URLSearchParams({ user: 'testing', password: 'test' }),
  credentials: 'include',
})

test('fetches files into the store', async () => {
  await fileBrowserStore.fetchFiles("/")

  expect(fileBrowserStore.loading).toBe(false)
  expect(fileBrowserStore.error).toBeNull()
  console.log(fileBrowserStore.items);
  // expect(fileBrowserStore.items).toEqual(expectedItems)
})
// test('get request to /api/users and see if it returns a successful response', async () => {
//   const response = await fetch('/api/users')
 

//   console.log('Result of /api/users:', {
//     status: response.status,
//     ok: response.ok,
//   })
//   const body = await response.json()
//   console.log('body: ', body);

//   expect(response.ok).toBe(true)
// })