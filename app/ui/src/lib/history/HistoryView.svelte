<script lang="ts">
  // Vue « Historique » : les dernières transformations, en lecture seule. Se met à
  // jour à chaque nouvelle entrée poussée par l'appli.
  import { onMount } from 'svelte'
  import { history, onHistoryChanged, type HistoryEntry } from '$lib/backend/history'

  /** Au-delà, un texte n'est montré qu'en partie (copies de centaines de Ko). */
  const SHOWN = 4000

  let entries = $state<HistoryEntry[] | null>(null)
  let failure = $state<string | null>(null)

  onMount(() => {
    let stop: (() => void) | undefined
    let gone = false
    ;(async () => {
      try {
        // Écouter avant de lire : une entrée entre les deux n'est pas perdue.
        const unlisten = await onHistoryChanged((next) => (entries = next))
        if (gone) return unlisten()
        stop = unlisten
        const current = await history()
        entries ??= current
      } catch (err) {
        failure = String(err)
      }
    })()
    return () => {
      gone = true
      stop?.()
    }
  })

  const time = new Intl.DateTimeFormat('fr-FR', { timeStyle: 'medium' })
  const dateTime = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'medium', timeStyle: 'medium' })

  /** L'heure seule pour une copie d'aujourd'hui, sinon la date et l'heure. */
  function when(at: number) {
    const today = new Date().toDateString() === new Date(at).toDateString()
    return (today ? time : dateTime).format(at)
  }

  const labels = {
    success: { text: 'Succès', class: 'bg-secondary text-secondary-foreground' },
    failure: { text: 'Échec', class: 'bg-destructive/10 text-destructive' },
    cancelled: { text: 'Annulé', class: 'bg-muted text-muted-foreground' },
  } as const

  /** L'étape en cause, sinon le message seul (résultat vide), avec une majuscule. */
  function failureText(step: string | null, message: string) {
    if (step) return `Étape « ${step} » : ${message}`
    return message.charAt(0).toUpperCase() + message.slice(1)
  }

  /** Marque d'un texte déjà coupé par l'appli (65 536 caractères au plus). */
  const CUT = /\n\[… (\d+) caractères de plus\]$/

  /**
   * Les [SHOWN] premiers caractères (points de code, comme l'appli : jamais un
   * emoji coupé en deux), puis combien il en manque, y compris ceux que l'appli
   * a déjà retirés.
   */
  function shown(text: string) {
    const cut = text.match(CUT)
    const body = cut ? text.slice(0, cut.index) : text
    const chars = Array.from(body)
    if (chars.length <= SHOWN) return text
    const missing = chars.length - SHOWN + (cut ? Number(cut[1]) : 0)
    return `${chars.slice(0, SHOWN).join('')}\n[… ${missing} caractères de plus]`
  }
</script>

{#snippet block(label: string, text: string)}
  <div class="flex flex-col gap-1">
    <span class="text-xs text-muted-foreground">{label}</span>
    {#if text}
      <pre class="max-h-32 overflow-auto rounded-md bg-muted/50 p-2 font-mono text-xs whitespace-pre-wrap break-all">{shown(text)}</pre>
    {:else}
      <span class="text-xs text-muted-foreground italic">(vide)</span>
    {/if}
  </div>
{/snippet}

<div class="flex flex-col gap-4 py-4">
  {#if failure}
    <p class="text-destructive">Impossible de lire l'historique : {failure}</p>
  {:else if entries}
    <p class="text-xs text-muted-foreground">
      Les 50 dernières transformations, la plus récente en haut. Gardées en mémoire seulement :
      l'historique disparaît au redémarrage.
    </p>
    {#if entries.length === 0}
      <p class="text-muted-foreground">Aucune transformation depuis le lancement.</p>
    {:else}
      <ol class="flex flex-col gap-3">
        {#each entries as entry, i (i)}
          <li class="flex flex-col gap-2 rounded-md border p-3">
            <div class="flex flex-wrap items-center gap-2">
              <span class="text-sm text-muted-foreground tabular-nums">{when(entry.at)}</span>
              <span class="rounded-md px-2 py-0.5 text-xs font-medium {labels[entry.status].class}">
                {labels[entry.status].text}
              </span>
              <span class="font-medium break-words">{entry.pipeline}</span>
              <code class="min-w-0 font-mono text-xs break-all text-muted-foreground">{entry.rule}</code>
            </div>
            {@render block('Original', entry.original)}
            {#if entry.status === 'success'}
              {#if entry.result === entry.original}
                <p class="text-xs text-muted-foreground">
                  Résultat identique à l'Original : le presse-papier n'est pas touché.
                </p>
              {:else}
                {@render block('Résultat', entry.result)}
              {/if}
            {:else if entry.status === 'failure'}
              <div role="alert" class="flex flex-col gap-2 rounded-md border border-destructive/50 p-2">
                <p class="text-sm text-destructive">{failureText(entry.step, entry.message)}</p>
                {#if entry.stderr}
                  {@render block("Sortie d'erreur", entry.stderr)}
                {/if}
                <p class="text-xs text-muted-foreground">Le presse-papier n'est pas touché.</p>
              </div>
            {:else}
              <p class="text-xs text-muted-foreground">
                Une copie plus récente est arrivée avant la fin : le résultat est jeté.
              </p>
            {/if}
          </li>
        {/each}
      </ol>
    {/if}
  {:else}
    <p class="text-muted-foreground">Chargement…</p>
  {/if}
</div>
