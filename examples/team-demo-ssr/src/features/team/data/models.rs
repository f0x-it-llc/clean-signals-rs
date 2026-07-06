//! Wire-format DTOs. Unlike the CSR demo's `MemberRow` (a fake in-process
//! row), this DTO actually crosses a process boundary — it is the payload the
//! `#[server]` functions (de)serialize over HTTP — so it derives `serde`. The
//! domain [`Member`] entity still never touches serde or the wire (see
//! `templates/AGENTS.md` data rules).

use crate::features::team::domain::entities::Member;
use serde::{Deserialize, Serialize};

/// Wire representation of a member, as the server functions hand it back and
/// forth. Never leaves the data layer — the repository converts it to the
/// domain [`Member`] at the boundary.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemberDto {
    pub id: String,
    pub name: String,
    pub role: String,
    pub email: String,
}

impl From<MemberDto> for Member {
    fn from(dto: MemberDto) -> Self {
        Member {
            id: dto.id,
            name: dto.name,
            role: dto.role,
            email: dto.email,
        }
    }
}

impl From<Member> for MemberDto {
    fn from(member: Member) -> Self {
        MemberDto {
            id: member.id,
            name: member.name,
            role: member.role,
            email: member.email,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_dto_converts_to_domain_entity() {
        let dto = MemberDto {
            id: "u1".to_string(),
            name: "Ava Chen".to_string(),
            role: "Mobile Engineer".to_string(),
            email: "ava@team.dev".to_string(),
        };
        let member: Member = dto.into();
        assert_eq!(member.id, "u1");
        assert_eq!(member.name, "Ava Chen");
    }

    #[test]
    fn member_round_trips_through_dto() {
        let member = Member {
            id: "u1".to_string(),
            name: "Ava Chen".to_string(),
            role: "Mobile Engineer".to_string(),
            email: "ava@team.dev".to_string(),
        };
        let dto: MemberDto = member.clone().into();
        let back: Member = dto.into();
        assert_eq!(member, back);
    }
}
