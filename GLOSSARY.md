# UltraClipboard

Petit outil d'arrière-plan qui transforme le contenu textuel du presse-papier selon des règles définies par l'utilisateur.

## Language

**Transformation**:
Une opération qui prend un texte et rend un texte (ex. `trim`, `upper`).
_Avoid_: Module, plugin, transformer

**Pipeline**:
Une suite ordonnée et nommée de Transformations, appliquées l'une après l'autre.
_Avoid_: Chaîne, workflow

**Étape**:
Une Transformation placée dans un Pipeline, avec ses paramètres.
_Avoid_: Step, maillon

**Règle**:
L'association d'un motif sur le contenu copié et d'un Pipeline à appliquer quand ce motif correspond.
_Avoid_: Trigger, condition

**Recette**:
Une Règle et son Pipeline, prêts à copier dans la config.
_Avoid_: Preset, snippet

**Original**:
Le contenu du presse-papier tel qu'il était avant la dernière Transformation appliquée.
_Avoid_: Négatif, backup
