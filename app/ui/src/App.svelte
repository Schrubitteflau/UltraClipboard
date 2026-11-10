<script lang="ts">
  import * as Tabs from '$lib/components/ui/tabs'
  import RulesView from '$lib/rules/RulesView.svelte'
  import HistoryView from '$lib/history/HistoryView.svelte'
</script>

<main class="flex h-screen flex-col p-4">
  <Tabs.Root value="rules" class="min-h-0 flex-1">
    <Tabs.List>
      <Tabs.Trigger value="rules">Règles</Tabs.Trigger>
      <Tabs.Trigger value="history">Historique</Tabs.Trigger>
    </Tabs.List>
    <Tabs.Content value="rules" class="min-h-0 overflow-y-auto"><RulesView /></Tabs.Content>
    <Tabs.Content value="history" class="min-h-0 overflow-y-auto"><HistoryView /></Tabs.Content>
  </Tabs.Root>
</main>
