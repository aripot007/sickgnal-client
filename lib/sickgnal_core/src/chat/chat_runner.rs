use futures::StreamExt;
use futures::future::BoxFuture;

pub struct ChatRunner {
    /// La tâche de réception réseau (E2E -> Local)
    pub recv_task: BoxFuture<'static, ()>,
    /// La tâche d'envoi réseau (Local -> E2E)
    pub send_task: BoxFuture<'static, ()>,
    /// La tâche de logique métier (Traitement des messages, stockage BDD)
    pub logic_task: BoxFuture<'static, ()>,
}
