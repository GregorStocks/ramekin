import { createSignal, For } from 'solid-js'
import './App.css'
import { DefaultApi, Configuration } from './generated/api'

// Create a configured API client
const api = new DefaultApi(new Configuration({
  basePath: '', // Empty because we're using the proxy
}))

function App() {
  const [garbageLists, setGarbageLists] = createSignal<string[][]>([])
  const [loading, setLoading] = createSignal(false)

  const fetchGarbages = async () => {
    setLoading(true)
    try {
      const data = await api.getGarbages()
      setGarbageLists([...garbageLists(), data.garbages])
    } catch (error) {
      console.error('Failed to fetch garbages:', error)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div style={{ padding: '2rem' }}>
      <h1>Ramekin Garbages</h1>

      <button
        onClick={fetchGarbages}
        disabled={loading()}
        style={{
          padding: '0.5rem 1rem',
          'font-size': '1rem',
          cursor: loading() ? 'not-allowed' : 'pointer'
        }}
      >
        {loading() ? 'Loading...' : 'Fetch Garbages'}
      </button>

      <div style={{ 'margin-top': '2rem' }}>
        <For each={garbageLists()}>
          {(garbages, index) => (
            <div style={{
              border: '1px solid #ccc',
              padding: '1rem',
              'margin-bottom': '1rem',
              'border-radius': '4px'
            }}>
              <h3>Fetch #{index() + 1}</h3>
              <ul>
                <For each={garbages}>
                  {(garbage) => <li>{garbage}</li>}
                </For>
              </ul>
            </div>
          )}
        </For>
      </div>
    </div>
  )
}

export default App
