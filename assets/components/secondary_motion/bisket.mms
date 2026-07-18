import { soft_hair_chain, soft_hair_chain_4, soft_bust_chain } from "spring_bone_presets.mms"

// Bisket 11 heuristic. Passing false selects these tuned defaults; it does not
// disable secondary motion. Override selectors without copying the settings:
// bisket_secondary_motion({ hair_chains = [...] hair_chains_4 = [...] bust_chains = [...] })
export fn bisket_secondary_motion(config) {
    let hair_chains = [
        ["[name='J_Sec_Hair1_02']", "[name='J_Sec_Hair2_02']", "[name='J_Sec_Hair3_02']"],
        ["[name='J_Sec_Hair1_03']", "[name='J_Sec_Hair2_03']", "[name='J_Sec_Hair3_03']"],
        ["[name='J_Sec_Hair1_08']", "[name='J_Sec_Hair2_08']", "[name='J_Sec_Hair3_08']"],
        ["[name='J_Sec_Hair1_09']", "[name='J_Sec_Hair2_09']", "[name='J_Sec_Hair3_09']"],
        ["[name='J_Sec_Hair1_10']", "[name='J_Sec_Hair2_10']", "[name='J_Sec_Hair3_10']"],
        ["[name='J_Sec_Hair1_11']", "[name='J_Sec_Hair2_11']", "[name='J_Sec_Hair3_11']"],
        ["[name='J_Sec_Hair1_12']", "[name='J_Sec_Hair2_12']", "[name='J_Sec_Hair3_12']"],
        ["[name='J_Sec_Hair1_14']", "[name='J_Sec_Hair2_14']", "[name='J_Sec_Hair3_14']"],
    ]
    let bust_chains = [
        ["[name='J_Sec_L_Bust1']", "[name='J_Sec_L_Bust2']"],
        ["[name='J_Sec_R_Bust1']", "[name='J_Sec_R_Bust2']"],
    ]
    let hair_chains_4 = [
        ["[name='J_Sec_Hair1_01']", "[name='J_Sec_Hair2_01']", "[name='J_Sec_Hair3_01']", "[name='J_Sec_Hair4_01']"],
        ["[name='J_Sec_Hair1_04']", "[name='J_Sec_Hair2_04']", "[name='J_Sec_Hair3_04']", "[name='J_Sec_Hair4_04']"],
        ["[name='J_Sec_Hair1_05']", "[name='J_Sec_Hair2_05']", "[name='J_Sec_Hair3_05']", "[name='J_Sec_Hair4_05']"],
        ["[name='J_Sec_Hair1_06']", "[name='J_Sec_Hair2_06']", "[name='J_Sec_Hair3_06']", "[name='J_Sec_Hair4_06']"],
        ["[name='J_Sec_Hair1_07']", "[name='J_Sec_Hair2_07']", "[name='J_Sec_Hair3_07']", "[name='J_Sec_Hair4_07']"],
        ["[name='J_Sec_Hair1_13']", "[name='J_Sec_Hair2_13']", "[name='J_Sec_Hair3_13']", "[name='J_Sec_Hair4_13']"],
    ]
    if config {
        hair_chains = config.hair_chains
        bust_chains = config.bust_chains
        hair_chains_4 = config.hair_chains_4
    }

    return SecondaryMotion {
        for chain in hair_chains {
            soft_hair_chain({ root = chain[0] middle = chain[1] tip = chain[2] })
        }
        for chain in hair_chains_4 {
            soft_hair_chain_4({ root = chain[0] middle = chain[1] tip = chain[2] end = chain[3] })
        }
        for chain in bust_chains {
            soft_bust_chain({ root = chain[0] tip = chain[1] })
        }
    }
}
