# Transformations extensibles par commandes externes, pas par modules JS embarqués

Le cadrage initial (BMAD) prévoyait des modules JavaScript exécutés dans un moteur embarqué (`boa`), avec manifest `module.yaml` et écosystème `ultraclipboard-module-*`. Nous y renonçons : les Transformations sont soit intégrées au binaire Rust, soit une Transformation `command` qui lance n'importe quel exécutable (texte sur stdin, résultat sur stdout). C'est la philosophie Unix sans moteur, sandbox, manifest ni écosystème à maintenir, et l'utilisateur écrit ses scripts dans le langage de son choix.

## Consequences

- Pas d'isolation des scripts : une commande a les droits de l'utilisateur, comme n'importe quel script qu'il lance lui-même.
- Un processus par exécution de `command` ; acceptable pour une copie humaine.
- Un processus qu'une `command` détache de son groupe (`setsid`, démon par double fork) n'est pas tué à l'annulation ni au délai : seul le groupe de la commande l'est.
