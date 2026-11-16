use super::services::Surface;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Permissions,
    Login,
    Calendar,
    Imports,
    Final,
}

impl Step {
    pub fn surface(self) -> Surface {
        match self {
            Self::Permissions => Surface::Permissions,
            Self::Login => Surface::Account,
            Self::Calendar => Surface::Calendar,
            Self::Imports => Surface::Imports,
            Self::Final => Surface::Onboarding,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Permissions => "Permissions",
            Self::Login => "Sign in",
            Self::Calendar => "Calendar",
            Self::Imports => "Imports",
            Self::Final => "Get started",
        }
    }
}

pub struct Onboarding {
    pub steps: Vec<Step>,
    position: usize,
}

impl Onboarding {
    pub fn new(macos: bool) -> Self {
        let mut steps = vec![];
        if macos {
            steps.push(Step::Permissions);
        }
        steps.extend([Step::Login, Step::Calendar, Step::Imports, Step::Final]);
        Self { steps, position: 0 }
    }

    pub fn step(&self) -> Step {
        self.steps[self.position]
    }

    pub fn advance(&mut self, permissions_ready: bool) -> bool {
        if (self.step() == Step::Permissions && !permissions_ready)
            || self.position + 1 == self.steps.len()
        {
            return false;
        }
        self.position += 1;
        true
    }

    pub fn back(&mut self) -> bool {
        if self.position == 0 {
            return false;
        }
        self.position -= 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_permission_gate_cannot_be_skipped() {
        let mut flow = Onboarding::new(true);
        assert_eq!(flow.step(), Step::Permissions);
        assert!(!flow.advance(false));
        assert!(flow.advance(true));
        assert_eq!(flow.step(), Step::Login);
        assert!(flow.back());
        assert!(!flow.back());
    }

    #[test]
    fn other_platforms_reach_final_without_claiming_completion() {
        let mut flow = Onboarding::new(false);
        assert_eq!(
            flow.steps,
            vec![Step::Login, Step::Calendar, Step::Imports, Step::Final]
        );
        for _ in 0..3 {
            assert!(flow.advance(false));
        }
        assert_eq!(flow.step(), Step::Final);
        assert!(!flow.advance(true));
    }
}
