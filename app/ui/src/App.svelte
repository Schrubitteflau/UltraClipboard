<script lang="ts">
  import * as Tabs from '$lib/components/ui/tabs'
</script>

<main class="flex h-screen flex-col p-4">
  <Tabs.Root value="rules" class="flex-1">
    <Tabs.List>
      <Tabs.Trigger value="rules">Règles</Tabs.Trigger>
      <Tabs.Trigger value="history">Historique</Tabs.Trigger>
    </Tabs.List>
    <Tabs.Content value="rules"></Tabs.Content>
    <Tabs.Content value="history"></Tabs.Content>
  </Tabs.Root>
</main>
