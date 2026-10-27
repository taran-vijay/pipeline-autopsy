import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import App from './App'

describe('application startup', () => {
  it('shows the runs empty state before any pipelines have been executed', () => {
    render(<App />)

    const runs = screen.getByRole('region', { name: 'Pipeline runs' })
    expect(within(runs).getByText(/No runs yet/)).toBeVisible()
    expect(screen.getByRole('main')).toBeVisible()
  })
})
