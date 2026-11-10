<script lang="ts">
  // Vue « Règles » : ce que l'appli a chargé, en lecture seule. Se met à jour à
  // chaque chargement de config poussé par l'appli.
  import { onMount } from 'svelte'
  import {
    configPath,
    configState,
    onConfigChanged,
    type ConfigState,
    type Refusal,
  } from '$lib/backend/config'

  let loaded = $state<ConfigState | null>(null)
  let path = $state<string | null>(null)
  let failure = $state<string | null>(null)

  onMount(() => {
    let stop: (() => void) | undefined
    let gone = false
    ;(async () => {
      try {
        // Écouter avant de lire : un chargement entre les deux n'est pas perdu.
        const unlisten = await onConfigChanged((next) => (loaded = next))
        if (gone) return unlisten()
        stop = unlisten
        const [current, file] = await Promise.all([configState(), configPath()])
        loaded ??= current
        path = file
      } catch (err) {
        failure = String(err)
      }
    })()
    return () => {
      gone = true
      stop?.()
    }
  })

  function describe(error: Refusal) {
    return error.field ? `${error.field} : ${error.reason}` : error.reason
  }

  /** Ce que le refus change : selon sa cause, et qu'une config était déjà active ou non. */
  function consequence(state: ConfigState, error: Refusal) {
    const cause =
      error.kind === 'unreadable'
        ? 'Le fichier de config ne peut pas être lu.'
        : 'La config est refusée.'
    if (!state.loaded)
      return `${cause} Aucune Règle active : rien n'est transformé tant que le fichier n'est pas corrigé.`
    return error.kind === 'unreadable'
      ? `${cause} Les Règles ci-dessous sont celles chargées avant.`
      : "Le dernier enregistrement n'est pas appliqué : les Règles ci-dessous sont celles de la config active."
  }
</script>

<div class="flex flex-col gap-6 py-4">
  {#if failure}
    <p class="text-destructive">Impossible de lire la config chargée : {failure}</p>
  {:else if loaded}
    <section aria-label="Fichier de config" class="flex flex-col gap-2">
      <div class="flex flex-wrap items-center gap-2">
        {#if loaded.error}
          <span class="rounded-md bg-destructive/10 px-2 py-0.5 text-xs font-medium text-destructive">
            {loaded.error.kind === 'unreadable' ? 'Fichier illisible' : 'Config refusée'}
          </span>
        {:else}
          <span class="rounded-md bg-secondary px-2 py-0.5 text-xs font-medium text-secondary-foreground">
            Config valide
          </span>
        {/if}
        <code class="break-all text-xs text-muted-foreground">
          {path ?? 'aucun fichier (ni XDG_CONFIG_HOME ni HOME)'}
        </code>
      </div>
      {#if loaded.error}
        <div role="alert" class="rounded-md border border-destructive/50 p-3">
          <p class="overflow-x-auto font-mono text-xs whitespace-pre-wrap text-destructive">{describe(loaded.error)}</p>
          <p class="mt-2 text-xs text-muted-foreground">{consequence(loaded, loaded.error)}</p>
        </div>
      {/if}
    </section>

    <section aria-labelledby="rules-title" class="flex flex-col gap-2">
      <h2 id="rules-title" class="font-medium">Règles</h2>
      {#if loaded.rules.length === 0}
        <p class="text-muted-foreground">Aucune Règle active : aucune copie n'est transformée.</p>
      {:else}
        <p class="text-xs text-muted-foreground">
          La première Règle dont le motif est trouvé dans le texte copié applique son Pipeline.
        </p>
        <table class="w-full table-fixed text-left">
          <thead class="text-xs text-muted-foreground">
            <tr class="border-b">
              <th class="w-8 py-1 font-normal">#</th>
              <th class="py-1 font-normal">Motif</th>
              <th class="w-1/3 py-1 font-normal">Pipeline</th>
            </tr>
          </thead>
          <tbody>
            {#each loaded.rules as rule, i (i)}
              <tr class="border-b align-top last:border-0">
                <td class="py-1.5 text-muted-foreground">{i + 1}</td>
                <td class="py-1.5 pr-2"><code class="font-mono text-xs break-all">{rule.pattern}</code></td>
                <td class="py-1.5 break-words">{rule.pipeline}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>

    <section aria-labelledby="pipelines-title" class="flex flex-col gap-2">
      <h2 id="pipelines-title" class="font-medium">Pipelines</h2>
      {#if loaded.pipelines.length === 0}
        <p class="text-muted-foreground">Aucun Pipeline.</p>
      {:else}
        <ul class="flex flex-col gap-3">
          {#each loaded.pipelines as pipeline, i (i)}
            <li class="rounded-md border p-3">
              <p class="font-medium break-words">{pipeline.name}</p>
              <ol class="mt-1 flex flex-col gap-1">
                {#each pipeline.steps as step, i (i)}
                  <li class="flex gap-2">
                    <span class="w-5 shrink-0 text-muted-foreground">{i + 1}.</span>
                    <span class="min-w-0">
                      <span>{step.transformation}</span>
                      {#if step.params}
                        <code class="ml-1 font-mono text-xs break-all text-muted-foreground">{step.params}</code>
                      {/if}
                    </span>
                  </li>
                {/each}
              </ol>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {:else}
    <p class="text-muted-foreground">Chargement…</p>
  {/if}
</div>
